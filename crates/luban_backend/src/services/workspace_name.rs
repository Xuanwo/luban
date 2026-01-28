use bip39::Language;
use rand::{Rng as _, rngs::OsRng};

pub(super) fn generate_workspace_name() -> anyhow::Result<String> {
    let words = Language::English.word_list();
    let mut rng = OsRng;
    let len = words.len();
    let w1 = words[rng.gen_range(0..len)];
    let w2 = words[rng.gen_range(0..len)];
    Ok(format!("{w1}-{w2}"))
}
