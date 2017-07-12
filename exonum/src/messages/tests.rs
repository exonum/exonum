use crypto::SecretKey;

#[test]
fn test_message_without_fields() {
    message! {
        struct NoFields {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 0;
        }
    }
    drop(NoFields::new(&SecretKey::new([1; 64])));
}
