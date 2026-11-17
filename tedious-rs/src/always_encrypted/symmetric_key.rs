#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymmetricKey {
    root_key: Vec<u8>,
}

impl SymmetricKey {
    pub fn new(root_key: Vec<u8>) -> Result<Self, String> {
        if root_key.is_empty() {
            return Err("Empty column encryption key specified.".to_owned());
        }
        Ok(Self { root_key })
    }

    pub fn root_key(&self) -> &[u8] {
        &self.root_key
    }

    pub fn zero_out_key(&mut self) {
        self.root_key.fill(0);
    }
}

impl Drop for SymmetricKey {
    fn drop(&mut self) {
        self.zero_out_key();
    }
}
