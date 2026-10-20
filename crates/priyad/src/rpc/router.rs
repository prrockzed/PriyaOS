//! [`Router`]: method names to typed handlers.
//!
//! A handler takes its method's `Params` and returns its `Result` or an
//! [`ApiError`]; the router does the JSON-RPC around it. Requests on one
//! connection are answered in order; a batch is answered as one array (or
//! not at all if it held only notifications). A method that streams
//! notifications is not run when called as a notification: the client
//! would never learn which stream is its own.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use priya_api::error::ApiError;
use priya_api::framing::encode;
use priya_api::methods::{Method, decode_params};
use priya_api::rpc::{Id, Incoming, Response, RpcError, parse_line, parse_request};
use serde::Serialize;
use serde_json::Value;

use super::Session;

type Reply = Result<Value, RpcError>;
type Handler<C> = Arc<
    dyn Fn(Arc<C>, Option<Value>, Session) -> Pin<Box<dyn Future<Output = Reply> + Send>>
        + Send
        + Sync,
>;

/// The methods a daemon serves, each with its context `C`.
pub struct Router<C> {
    methods: BTreeMap<&'static str, Served<C>>,
}

struct Served<C> {
    handler: Handler<C>,
    /// Whether the method answers with a stream of notifications.
    streams: bool,
}

impl<C> Default for Router<C> {
    fn default() -> Self {
        Self {
            methods: BTreeMap::new(),
        }
    }
}

impl<C: Send + Sync + 'static> Router<C> {
    /// Serves method `M` with `handler`.
    pub fn add<M, F, Fut>(&mut self, handler: F) -> &mut Self
    where
        M: Method,
        F: Fn(Arc<C>, M::Params, Session) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<M::Result, ApiError>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let served = Served {
            handler: Arc::new(move |context, params, session| {
                let handler = handler.clone();
                Box::pin(async move {
                    let params: M::Params = decode_params(params)?;
                    let result = handler(context, params, session)
                        .await
                        .map_err(RpcError::from_api)?;
                    serde_json::to_value(result).map_err(|e| RpcError::internal(e.to_string()))
                })
            }),
            streams: M::NOTIFICATION.is_some(),
        };
        self.methods.insert(M::NAME, served);
        self
    }

    /// The names served, sorted.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.methods.keys().copied()
    }

    /// Answers one line from a connection: the response line, or none when
    /// it held only notifications.
    pub async fn handle(
        &self,
        context: &Arc<C>,
        session: &Session,
        line: &[u8],
    ) -> Option<Vec<u8>> {
        match parse_line(line) {
            Err(error) => Some(frame(&Response::err(Id::Null, error), Id::Null)),
            Ok(Incoming::Single(message)) => {
                let response = self.one(context, session, message).await?;
                let id = response.id.clone();
                Some(frame(&response, id))
            }
            Ok(Incoming::Batch(messages)) => {
                let mut answers = Vec::new();
                for message in messages {
                    answers.extend(self.one(context, session, message).await);
                }
                (!answers.is_empty()).then(|| frame(&answers, Id::Null))
            }
        }
    }

    /// Calls one method directly, as a module's `host.call` does: no line
    /// and no connection, so any notification it sends goes nowhere.
    pub async fn call(&self, context: &Arc<C>, method: &str, params: Value) -> Reply {
        let Some(served) = self.methods.get(method) else {
            return Err(RpcError::method_not_found(method));
        };
        let (out, _) = tokio::sync::mpsc::channel(1);
        (served.handler)(context.clone(), Some(params), Session::new(out)).await
    }

    /// Answers a line that was too long to read.
    pub fn too_long() -> Vec<u8> {
        let error = RpcError::invalid_request("the request is longer than 16 MiB");
        frame(&Response::err(Id::Null, error), Id::Null)
    }

    async fn one(&self, context: &Arc<C>, session: &Session, message: Value) -> Option<Response> {
        let request = match parse_request(message) {
            Ok(request) => request,
            Err((id, error)) => return Some(Response::err(id, error)),
        };
        let outcome = match self.methods.get(request.method.as_str()) {
            None => Err(RpcError::method_not_found(&request.method)),
            Some(served) if served.streams && request.id.is_none() => return None,
            Some(served) => {
                (served.handler)(context.clone(), request.params, session.clone()).await
            }
        };
        // A notification gets no answer, even an error.
        let id = request.id?;
        Some(match outcome {
            Ok(result) => Response::ok(id, result),
            Err(error) => Response::err(id, error),
        })
    }
}

/// `reply` as a line; one too big to send becomes an error to `id`.
fn frame(reply: &impl Serialize, id: Id) -> Vec<u8> {
    encode(reply).unwrap_or_else(|e| {
        let error = RpcError::internal(format!("the reply couldn't be sent: {e}"));
        encode(&Response::err(id, error)).unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use priya_api::error::ErrorKind;
    use priya_api::methods::{audit, events, system};
    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;

    /// A router with two methods: one that works, one that always fails,
    /// and a stream that must never start without an id.
    fn router() -> Router<()> {
        let mut router = Router::default();
        router
            .add::<events::Subscribe, _, _>(|_, _, _| async {
                panic!("a subscription nobody can recognise was started")
            })
            .add::<system::Methods, _, _>(|_, _, _| async {
                Ok(system::MethodsResult {
                    methods: Vec::new(),
                })
            })
            .add::<audit::Verify, _, _>(|_, params, _| async move {
                Err(ApiError::new(
                    ErrorKind::NotFound,
                    format!("There is no event {}.", params.from.unwrap_or(0)),
                ))
            });
        router
    }

    async fn ask(router: &Router<()>, line: &str) -> Option<Value> {
        let (tx, _rx) = mpsc::channel(4);
        let reply = router
            .handle(&Arc::new(()), &Session::new(tx), line.as_bytes())
            .await?;
        assert_eq!(reply.last(), Some(&b'\n'));
        Some(serde_json::from_slice(&reply).unwrap())
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.7
    // regression: 0.7
    #[tokio::test]
    async fn router_maps_errors_to_jsonrpc_codes_with_user_message() {
        let router = router();
        let reply = ask(
            &router,
            r#"{"jsonrpc":"2.0","id":1,"method":"v1.audit.verify","params":{"from":9}}"#,
        )
        .await
        .unwrap();
        assert_eq!(reply["error"]["code"], -32001);
        assert_eq!(reply["error"]["data"]["kind"], "not_found");
        assert_eq!(
            reply["error"]["data"]["user_message"],
            "There is no event 9."
        );
        assert_eq!(reply["error"]["data"]["retryable"], false);

        let bad_params = ask(
            &router,
            r#"{"jsonrpc":"2.0","id":2,"method":"v1.audit.verify","params":{"from":"x"}}"#,
        )
        .await
        .unwrap();
        assert_eq!(bad_params["error"]["code"], -32602);
        assert_eq!(bad_params["error"]["data"]["kind"], "invalid_input");
        let positional = ask(
            &router,
            r#"{"jsonrpc":"2.0","id":3,"method":"v1.audit.verify","params":[1]}"#,
        )
        .await
        .unwrap();
        assert_eq!(positional["error"]["code"], -32602);

        let parse = ask(&router, "{oops").await.unwrap();
        assert_eq!(
            (parse["id"].clone(), parse["error"]["code"].clone()),
            (Value::Null, json!(-32700))
        );
        let invalid = ask(&router, r#"{"jsonrpc":"2.0","id":4}"#).await.unwrap();
        assert_eq!(
            (invalid["id"].clone(), invalid["error"]["code"].clone()),
            (json!(4), json!(-32600))
        );
        assert!(
            !invalid["error"]["data"]["user_message"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.7
    // regression: 0.7
    #[tokio::test]
    async fn router_rejects_unknown_method_with_32601() {
        let reply = ask(
            &router(),
            r#"{"jsonrpc":"2.0","id":"a","method":"v1.nope"}"#,
        )
        .await
        .unwrap();
        assert_eq!(reply["id"], "a");
        assert_eq!(reply["error"]["code"], -32601);
        assert_eq!(reply["error"]["data"]["kind"], "not_found");
        // Absent and null params both mean none.
        let ok = ask(
            &router(),
            r#"{"jsonrpc":"2.0","id":5,"method":"v1.system.methods","params":null}"#,
        )
        .await
        .unwrap();
        assert_eq!(ok["result"], json!({"methods": []}));
    }

    // covers: CORE-04
    // suite: unit
    #[test]
    fn a_line_too_long_gets_an_invalid_request_error() {
        let reply: Value = serde_json::from_slice(&Router::<()>::too_long()).unwrap();
        assert_eq!(reply["id"], Value::Null);
        assert_eq!(reply["error"]["code"], -32600);
        assert_eq!(reply["error"]["data"]["kind"], "invalid_input");
    }

    // covers: CORE-04
    // suite: unit
    // acceptance: 0.7
    // regression: 0.7
    #[tokio::test]
    async fn batch_requests_answer_every_id() {
        let router = router();
        let reply = ask(
            &router,
            r#"[
                {"jsonrpc":"2.0","id":1,"method":"v1.system.methods"},
                {"jsonrpc":"2.0","method":"v1.system.methods"},
                {"jsonrpc":"2.0","id":2,"method":"v1.nope"},
                {"nonsense":true},
                {"jsonrpc":"2.0","id":3,"method":"v1.audit.verify"}
            ]"#,
        )
        .await
        .unwrap();
        let answers = reply.as_array().unwrap();
        let ids: Vec<Value> = answers.iter().map(|a| a["id"].clone()).collect();
        // The notification gets no answer; the unreadable one gets a null id.
        assert_eq!(ids, [json!(1), json!(2), Value::Null, json!(3)]);
        assert!(answers[0].get("result").is_some());
        assert_eq!(answers[1]["error"]["code"], -32601);
        assert_eq!(answers[2]["error"]["code"], -32600);
        assert_eq!(answers[3]["error"]["code"], -32001);

        // Only notifications: no reply at all.
        assert_eq!(
            ask(
                &router,
                r#"[{"jsonrpc":"2.0","method":"v1.system.methods"}]"#
            )
            .await,
            None
        );
        assert_eq!(
            ask(&router, r#"{"jsonrpc":"2.0","method":"v1.nope"}"#).await,
            None,
            "not even an error for a notification"
        );
        assert_eq!(
            ask(
                &router,
                r#"{"jsonrpc":"2.0","method":"v1.events.subscribe"}"#
            )
            .await,
            None,
            "a stream called as a notification isn't started"
        );
    }
}
