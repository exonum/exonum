pub fn bytes_to_hex<T: AsRef<[u8]> + ?Sized>(bytes: &T) -> String {
    let strs: Vec<String> = bytes.as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    strs.join("")
}
