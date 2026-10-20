.pragma library

// A pseudo-locale for clipping tests (0.11, DS-05): every letter accented
// and the text 40 % longer, as many translations are, in brackets so a cut
// end shows.
const ACCENTED = {
    "a": "á", "e": "é", "i": "í", "o": "ö", "u": "ü", "c": "ç", "n": "ñ", "y": "ý",
    "A": "Á", "E": "É", "I": "Í", "O": "Ö", "U": "Ü", "C": "Ç", "N": "Ñ", "Y": "Ý"
};

function pseudo(text) {
    const accented = text.replace(/[aeiouycnAEIOUYCN]/g, c => ACCENTED[c]);
    const extra = Math.ceil(text.length * 0.4);
    let pad = "";
    const filler = " ~ẍ";
    for (let i = 0; pad.length < extra; ++i)
        pad += filler[i % filler.length];
    return "[" + accented + pad + "]";
}
