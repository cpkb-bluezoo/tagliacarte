/*
 * crypto.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Matrix Olm/Megolm E2EE was previously implemented with vodozemac; that dependency
 * was removed. `CryptoMachine` is a private-field struct that cannot be constructed
 * from outside this module, so [`crate::protocol::matrix::MatrixStore::get_crypto`]
 * always returns `None` until another backend is integrated.
 */

/// Placeholder for a future Matrix E2EE implementation. Not constructible outside this module.
pub struct CryptoMachine {
    _private: (),
}

/// Fields for an `m.room.encrypted` Megolm-shaped event (encryption path disabled).
pub struct MegolmEncrypted {
    pub algorithm: String,
    pub sender_key: String,
    pub ciphertext: String,
    pub session_id: String,
    pub device_id: String,
}
