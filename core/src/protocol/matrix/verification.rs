/*
 * verification.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * JSON builders for `m.key.verification.*` events. SAS cryptography (previously
 * vodozemac) was removed; only wire-format helpers remain.
 */

use std::collections::HashMap;

use crate::json::JsonWriter;

/// SAS emoji table from the Matrix spec (64 entries).
pub const SAS_EMOJIS: [(&str, &str); 64] = [
    ("🐶", "Dog"),
    ("🐱", "Cat"),
    ("🦁", "Lion"),
    ("🐴", "Horse"),
    ("🦄", "Unicorn"),
    ("🐷", "Pig"),
    ("🐘", "Elephant"),
    ("🐰", "Rabbit"),
    ("🐼", "Panda"),
    ("🐓", "Rooster"),
    ("🐧", "Penguin"),
    ("🐢", "Turtle"),
    ("🐟", "Fish"),
    ("🐙", "Octopus"),
    ("🦋", "Butterfly"),
    ("🌷", "Flower"),
    ("🌳", "Tree"),
    ("🌵", "Cactus"),
    ("🍄", "Mushroom"),
    ("🌏", "Globe"),
    ("🌙", "Moon"),
    ("☁️", "Cloud"),
    ("🔥", "Fire"),
    ("🍌", "Banana"),
    ("🍎", "Apple"),
    ("🍓", "Strawberry"),
    ("🌽", "Corn"),
    ("🍕", "Pizza"),
    ("🎂", "Cake"),
    ("❤️", "Heart"),
    ("😀", "Smiley"),
    ("🤖", "Robot"),
    ("🎩", "Hat"),
    ("👓", "Glasses"),
    ("🔧", "Spanner"),
    ("🎅", "Santa"),
    ("👍", "Thumbs Up"),
    ("☂️", "Umbrella"),
    ("⌛", "Hourglass"),
    ("⏰", "Clock"),
    ("🎁", "Gift"),
    ("💡", "Light Bulb"),
    ("📕", "Book"),
    ("✏️", "Pencil"),
    ("📎", "Paperclip"),
    ("✂️", "Scissors"),
    ("🔒", "Lock"),
    ("🔑", "Key"),
    ("🔨", "Hammer"),
    ("☎️", "Telephone"),
    ("🏁", "Flag"),
    ("🚂", "Train"),
    ("🚲", "Bicycle"),
    ("✈️", "Aeroplane"),
    ("🚀", "Rocket"),
    ("🏆", "Trophy"),
    ("⚽", "Ball"),
    ("🎸", "Guitar"),
    ("🎺", "Trumpet"),
    ("🔔", "Bell"),
    ("⚓", "Anchor"),
    ("🎧", "Headphones"),
    ("📁", "Folder"),
    ("📌", "Pin"),
];

pub fn build_verification_request_event(
    transaction_id: &str,
    from_device: &str,
    methods: &[&str],
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("from_device");
    w.write_string(from_device);
    w.write_key("methods");
    w.write_start_array();
    for m in methods {
        w.write_string(m);
    }
    w.write_end_array();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_ready_event(
    transaction_id: &str,
    from_device: &str,
    methods: &[&str],
) -> Vec<u8> {
    build_verification_request_event(transaction_id, from_device, methods)
}

pub fn build_verification_start_event(transaction_id: &str, from_device: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("from_device");
    w.write_string(from_device);
    w.write_key("method");
    w.write_string("m.sas.v1");
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("key_agreement_protocols");
    w.write_start_array();
    w.write_string("curve25519-hkdf-sha256");
    w.write_end_array();
    w.write_key("hashes");
    w.write_start_array();
    w.write_string("sha256");
    w.write_end_array();
    w.write_key("message_authentication_codes");
    w.write_start_array();
    w.write_string("hkdf-hmac-sha256.v2");
    w.write_end_array();
    w.write_key("short_authentication_string");
    w.write_start_array();
    w.write_string("emoji");
    w.write_string("decimal");
    w.write_end_array();
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_accept_event(transaction_id: &str, commitment: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("method");
    w.write_string("m.sas.v1");
    w.write_key("key_agreement_protocol");
    w.write_string("curve25519-hkdf-sha256");
    w.write_key("hash");
    w.write_string("sha256");
    w.write_key("message_authentication_code");
    w.write_string("hkdf-hmac-sha256.v2");
    w.write_key("short_authentication_string");
    w.write_start_array();
    w.write_string("emoji");
    w.write_string("decimal");
    w.write_end_array();
    w.write_key("commitment");
    w.write_string(commitment);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_key_event(transaction_id: &str, key_b64: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("key");
    w.write_string(key_b64);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_mac_event(
    transaction_id: &str,
    mac: &HashMap<String, String>,
    keys: &str,
) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("mac");
    w.write_start_object();
    for (k, v) in mac {
        w.write_key(k);
        w.write_string(v);
    }
    w.write_end_object();
    w.write_key("keys");
    w.write_string(keys);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_done_event(transaction_id: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_end_object();
    w.take_buffer().to_vec()
}

pub fn build_verification_cancel_event(transaction_id: &str, code: &str, reason: &str) -> Vec<u8> {
    let mut w = JsonWriter::new();
    w.write_start_object();
    w.write_key("transaction_id");
    w.write_string(transaction_id);
    w.write_key("code");
    w.write_string(code);
    w.write_key("reason");
    w.write_string(reason);
    w.write_end_object();
    w.take_buffer().to_vec()
}
