use twerk_web::api::domain::{Page, PageError, PageSize, PageSizeError};

#[kani::proof]
fn page_rejects_zero() {
    let result = Page::new(0);
    assert!(
        matches!(result, Err(PageError::TooSmall)),
        "Page(0) must be rejected"
    );
}

#[kani::proof]
fn page_accepts_one() {
    let result = Page::new(1);
    assert!(result.is_ok(), "Page(1) must be accepted");
    assert_eq!(result.unwrap().get(), 1);
}

#[kani::proof]
fn page_size_rejects_zero() {
    let result = PageSize::new(0);
    assert!(
        matches!(result, Err(PageSizeError::TooSmall)),
        "PageSize(0) must be rejected"
    );
}

#[kani::proof]
fn page_size_accepts_one() {
    let result = PageSize::new(1);
    assert!(result.is_ok(), "PageSize(1) must be accepted");
    assert_eq!(result.unwrap().get(), 1);
}

#[kani::proof]
fn page_size_rejects_over_100() {
    let result = PageSize::new(101);
    assert!(
        matches!(result, Err(PageSizeError::TooLarge { .. })),
        "PageSize(101) must be rejected"
    );
}

#[kani::proof]
fn page_size_accepts_100() {
    let result = PageSize::new(100);
    assert!(result.is_ok(), "PageSize(100) must be accepted");
    assert_eq!(result.unwrap().get(), 100);
}
