use bip39::Language;
use rand::{Rng as _, rngs::OsRng};

pub fn generate_workspace_name() -> anyhow::Result<String> {
    let words = Language::English.word_list();
    let mut rng = OsRng;
    let w1 = words[rng.gen_range(0..words.len())];
    let w2 = words[rng.gen_range(0..words.len())];
    Ok(format!("{w1}-{w2}"))
}
