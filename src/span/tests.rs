use super::*;

#[test]
fn test_span_creation() {
    let span = Span::new(0, 5);
    assert_eq!(span.start, 0);
    assert_eq!(span.stop, 5);
    assert_eq!(span.len(), 5);
}

#[test]
fn test_len() {
    let s = Span::new(3, 10);
    assert_eq!(s.len(), 7);
}

#[test]
fn test_is_empty() {
    let a = Span::new(0, 0);
    assert_eq!(a.is_empty(), true);

    let b = Span::new(0, 1);
    assert_eq!(b.is_empty(), false);
}

#[test]
fn test_contains() {
    let s = Span::new(2, 5);
    assert!(s.contains(2));
    assert!(s.contains(4));
    assert!(!s.contains(5));
}

#[test]
fn test_overlaps() {
    let a = Span::new(0, 5);
    let b = Span::new(3, 10);
    assert!(a.overlaps(&b));

    let c = Span::new(5, 8);
    assert!(!a.overlaps(&c));
}

#[test]
fn test_merge() {
    let a = Span::new(0, 5);
    let b = Span::new(3, 10);

    let merged = a.merge(&b);
    assert_eq!(merged, Some(Span::new(0, 10)));

    let c = Span::new(5, 8);
    assert_eq!(a.merge(&c), Some(Span::new(0, 8)));

    let d = Span::new(6, 7);
    assert_eq!(a.merge(&d), None);
}
