//! What the model is told: the system prompt, and the conversation so far.

use time::Date;
use time::macros::format_description;

use crate::model::Message;

/// The prompt built into priyad, used when the installed file
/// (`/usr/share/priya/prompts/answer.md`) can't be read.
pub const BUILT_IN: &str = include_str!("../../../../data/prompts/answer.md");

/// At most this many earlier questions and answers go with a follow-up.
pub const MAX_TURNS: usize = 10;
/// The most text, in characters, sent with a question, the system prompt
/// included; the oldest turns go first. That is about 6,000 tokens of
/// English, but up to 24,000 for Hindi or Chinese, where a character is
/// nearer a token: Phase 1 counts tokens per model instead.
pub const MAX_CHARS: usize = 24_000;

/// `template` with `{date}` replaced by today's date, written out in full
/// ("Tuesday 20 October 2026") so the model can't misread it.
pub fn system_prompt(template: &str, today: Date) -> String {
    let format = format_description!("[weekday] [day padding:none] [month repr:long] [year]");
    let date = today.format(format).unwrap_or_else(|_| today.to_string());
    if template.contains("{date}") {
        template.replace("{date}", &date)
    } else {
        // A template edited without the placeholder still gets the date.
        format!("{}\n\nToday is {date}.", template.trim_end())
    }
}

/// One earlier question and its answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub question: String,
    pub answer: String,
}

/// The messages for `question`, after as many of the most recent `earlier`
/// turns (oldest first) as fit in [`MAX_TURNS`] and [`MAX_CHARS`] with the
/// `system` prompt. The question itself always goes, however long. Turns
/// without an answer (stopped or failed) are left out: some servers refuse
/// an empty assistant message.
pub fn conversation(system: &str, earlier: &[Turn], question: &str) -> Vec<Message> {
    let mut budget = MAX_CHARS.saturating_sub(question.chars().count() + system.chars().count());
    let answered: Vec<&Turn> = earlier
        .iter()
        .filter(|t| !t.answer.trim().is_empty())
        .collect();
    let kept = answered
        .iter()
        .rev()
        .take(MAX_TURNS)
        .take_while(|turn| {
            let size = turn.question.chars().count() + turn.answer.chars().count();
            let fits = size <= budget;
            budget = budget.saturating_sub(size);
            fits
        })
        .count();
    answered[answered.len() - kept..]
        .iter()
        .flat_map(|turn| {
            [
                Message::user(&turn.question),
                Message::assistant(&turn.answer),
            ]
        })
        .chain([Message::user(question)])
        .collect()
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::model::Role;

    // covers: AI-21
    // suite: unit
    // acceptance: 0.12
    #[test]
    fn system_prompt_includes_date() {
        let prompt = system_prompt(BUILT_IN, date!(2026 - 10 - 20));
        assert!(
            prompt.contains("Today is Tuesday 20 October 2026."),
            "{prompt}"
        );
        assert!(!prompt.contains("{date}"));
        assert!(prompt.starts_with("You are Priya"));
        let edited = system_prompt("Be terse.\n", date!(2026 - 01 - 05));
        assert_eq!(edited, "Be terse.\n\nToday is Monday 5 January 2026.");
    }

    fn turn(n: usize, size: usize) -> Turn {
        Turn {
            question: format!("q{n}"),
            answer: "a".repeat(size),
        }
    }

    // covers: AI-21
    // suite: unit
    // acceptance: 0.12
    #[test]
    fn conversation_context_drops_oldest_first() {
        // Twelve short turns: only the latest ten go, in order.
        let earlier: Vec<Turn> = (1..=12).map(|n| turn(n, 10)).collect();
        let messages = conversation("", &earlier, "now?");
        assert_eq!(messages.len(), 21);
        assert_eq!(messages[0].text, "q3");
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[19].text, "a".repeat(10));
        assert_eq!(messages[20].text, "now?");

        // Long answers: the oldest go first to stay within MAX_CHARS.
        let long: Vec<Turn> = (1..=4).map(|n| turn(n, 9_000)).collect();
        let messages = conversation("", &long, "now?");
        assert!(messages.iter().map(|m| m.text.len()).sum::<usize>() <= MAX_CHARS);
        let questions: Vec<&str> = messages
            .iter()
            .filter(|m| m.role == Role::User)
            .map(|m| m.text.as_str())
            .collect();
        assert_eq!(questions, ["q3", "q4", "now?"]);

        // A turn too big on its own stops older ones joining too.
        let mixed = vec![turn(1, 10), turn(2, MAX_CHARS), turn(3, 10)];
        let texts: Vec<String> = conversation("", &mixed, "now?")
            .into_iter()
            .map(|m| m.text)
            .collect();
        assert_eq!(texts, ["q3".to_owned(), "a".repeat(10), "now?".to_owned()]);

        // A question longer than the budget still goes, alone.
        let huge = "x".repeat(MAX_CHARS + 5);
        assert_eq!(conversation("", &earlier, &huge).len(), 1);
        assert_eq!(conversation("", &[], "hi").len(), 1);

        // The system prompt counts: with it, one turn fewer fits.
        let system = "s".repeat(9_000);
        let user_texts =
            |messages: Vec<Message>| messages.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_texts(conversation(&system, &long, "now?")), 2);

        // Unanswered turns are skipped, not sent as empty answers.
        let stopped = vec![turn(1, 10), turn(2, 0), turn(3, 10)];
        let texts: Vec<String> = conversation("", &stopped, "now?")
            .into_iter()
            .map(|m| m.text)
            .collect();
        assert_eq!(
            texts,
            [
                "q1".to_owned(),
                "a".repeat(10),
                "q3".into(),
                "a".repeat(10),
                "now?".into()
            ]
        );
    }
}
