// local_embed.rs
const LOCAL_DIMS: usize = 384;
pub const LOCAL_MODEL_ID: &str = "chebo-local-v1";

fn bucket_add(vec: &mut [f32], token: &str) {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash = hasher.finalize();
    let idx = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]) as usize % vec.len();
    let sign = if hash[4] & 1 == 0 { 1.0f32 } else { -1.0f32 };
    vec[idx] += sign;
}

fn l2_normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 { for x in vec.iter_mut() { *x /= norm; } }
}

pub fn embed_local(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0f32; LOCAL_DIMS];
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() { return vec; }
    let chars: Vec<char> = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    for i in 0..chars.len() {
        bucket_add(&mut vec, &chars[i..=i].iter().collect::<String>());
        if i + 1 < chars.len() { bucket_add(&mut vec, &chars[i..=i+1].iter().collect::<String>()); }
        if i + 2 < chars.len() { bucket_add(&mut vec, &chars[i..=i+2].iter().collect::<String>()); }
    }
    for word in normalized.split(|c: char| !c.is_alphanumeric()) {
        if word.len() >= 2 { bucket_add(&mut vec, word); }
    }
    l2_normalize(&mut vec);
    vec
}