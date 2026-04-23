use twerk_web::api::domain::{Password, Username, PasswordError, UsernameError};

#[kani::proof]
fn username_rejects_empty() {
    let result = Username::new("");
    assert!(
        matches!(result, Err(UsernameError::Empty)),
        "Empty username must be rejected"
    );
}

#[kani::proof]
fn username_rejects_too_short() {
    // 2 characters is below the minimum of 3
    let result = Username::new("ab");
    assert!(
        matches!(result, Err(UsernameError::LengthOutOfRange)),
        "Usernames shorter than 3 chars must be rejected"
    );
}

#[kani::proof]
fn username_rejects_too_long() {
    // 65 characters exceeds the maximum of 64
    let long_name = "a".repeat(65);
    let result = Username::new(&long_name);
    assert!(
        matches!(result, Err(UsernameError::LengthOutOfRange)),
        "Usernames longer than 64 chars must be rejected"
    );
}

#[kani::proof]
fn username_rejects_starting_digit() {
    let result = Username::new("1admin");
    assert!(
        matches!(result, Err(UsernameError::InvalidCharacter)),
        "Usernames starting with a digit must be rejected"
    );
}

#[kani::proof]
fn username_accepts_valid() {
    let result = Username::new("admin");
    assert!(result.is_ok(), "Valid username 'admin' must be accepted");
    assert_eq!(result.unwrap().as_str(), "admin");
}

#[kani::proof]
fn password_rejects_empty() {
    let result = Password::new("");
    assert!(
        matches!(result, Err(PasswordError::Empty)),
        "Empty password must be rejected"
    );
}

#[kani::proof]
fn password_rejects_too_short() {
    // 7 characters is below the minimum of 8
    let result = Password::new("1234567");
    assert!(
        matches!(result, Err(PasswordError::TooShort)),
        "Passwords shorter than 8 chars must be rejected"
    );
}

#[kani::proof]
fn password_accepts_8_chars() {
    // Exactly 8 characters is the minimum accepted length
    let result = Password::new("12345678");
    assert!(
        result.is_ok(),
        "8-character password must be accepted"
    );
    assert_eq!(result.unwrap().as_str(), "12345678");
}
