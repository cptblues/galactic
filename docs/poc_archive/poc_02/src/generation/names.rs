use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

const PREFIXES: &[&str] = &[
    "Al", "Ar", "Bel", "Cer", "Dra", "Eri", "Hel", "Kor", "Ly", "Nex", "Or", "Pra", "Sol", "Tal",
    "Vel",
];
const MIDDLES: &[&str] = &["a", "e", "i", "o", "u", "ae", "io", "ar", "en", "on"];
const SUFFIXES: &[&str] = &[
    "ia", "on", "us", "ar", "is", "ea", "Prime", "Minor", "Major",
];

pub fn unique_system_name(rng: &mut ChaCha8Rng, used: &mut HashSet<String>) -> String {
    for _ in 0..512 {
        let name = format!(
            "{}{}{}",
            PREFIXES[rng.random_range(0..PREFIXES.len())],
            MIDDLES[rng.random_range(0..MIDDLES.len())],
            SUFFIXES[rng.random_range(0..SUFFIXES.len())]
        );
        if used.insert(name.clone()) {
            return name;
        }
    }

    let mut suffix = used.len();
    loop {
        let name = format!("Nex{}{}", MIDDLES[suffix % MIDDLES.len()], suffix);
        suffix += 1;
        if used.insert(name.clone()) {
            return name;
        }
    }
}

pub fn roman(index: usize) -> &'static str {
    match index {
        0 => "I",
        1 => "II",
        2 => "III",
        3 => "IV",
        4 => "V",
        5 => "VI",
        6 => "VII",
        7 => "VIII",
        8 => "IX",
        _ => "X",
    }
}
