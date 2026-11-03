use crate::err::ColumnarError;
use crate::{
    AnyRleDecoder, AnyRleEncoder, BoolRleDecoder, BoolRleEncoder, DeltaRleDecoder, DeltaRleEncoder,
};

#[test]
fn test_rle_empty_and_single_element() {
    // Empty
    let encoder = AnyRleEncoder::<u32>::new();
    let buf = encoder.finish().expect("empty RLE should finish");
    let mut decoder = AnyRleDecoder::<u32>::new(&buf);
    let decoded = decoder.decode().expect("empty RLE should decode");
    assert!(decoded.is_empty());

    // Single element
    let mut encoder = AnyRleEncoder::<u32>::new();
    encoder.append(42).unwrap();
    let buf = encoder.finish().expect("single element RLE should finish");
    let mut decoder = AnyRleDecoder::<u32>::new(&buf);
    let decoded = decoder.decode().expect("single element RLE should decode");
    assert_eq!(decoded, vec![42]);
}

#[test]
fn test_bool_rle_patterns() {
    // Alternating
    let input = vec![true, false, true, false, true];
    let mut encoder = BoolRleEncoder::new();
    for &b in &input {
        encoder.append(b).unwrap();
    }
    let buf = encoder.finish().expect("alternating bools should finish");
    let mut decoder = BoolRleDecoder::new(&buf);
    assert_eq!(decoder.decode().unwrap(), input);

    // Homogeneous true
    let input_true = vec![true; 100];
    let mut encoder = BoolRleEncoder::new();
    for &b in &input_true {
        encoder.append(b).unwrap();
    }
    let buf = encoder.finish().unwrap();
    let mut decoder = BoolRleDecoder::new(&buf);
    assert_eq!(decoder.decode().unwrap(), input_true);

    // Homogeneous false
    let input_false = vec![false; 50];
    let mut encoder = BoolRleEncoder::new();
    for &b in &input_false {
        encoder.append(b).unwrap();
    }
    let buf = encoder.finish().unwrap();
    let mut decoder = BoolRleDecoder::new(&buf);
    assert_eq!(decoder.decode().unwrap(), input_false);
}

#[test]
fn test_delta_rle_monotonic() {
    let input = vec![100u64, 105, 110, 115, 120];
    let mut encoder = DeltaRleEncoder::new();
    for &val in &input {
        encoder.append(val).unwrap();
    }
    let buf = encoder.finish().expect("delta RLE should finish");
    let mut decoder = DeltaRleDecoder::new(&buf);
    let decoded: Vec<u64> = decoder.decode().expect("delta RLE should decode");
    assert_eq!(decoded, input);
}

#[test]
fn test_columnar_error_formatting() {
    let rle_err = ColumnarError::RleEncodeError("corrupted stream".to_string());
    assert_eq!(
        format!("{rle_err}"),
        "`corrupted stream` during rle encoding"
    );

    let strategy_err = ColumnarError::InvalidStrategy(99);
    assert_eq!(format!("{strategy_err}"), "invalid strategy code `99`");

    let overflow_err = ColumnarError::OverflowError;
    assert_eq!(format!("{overflow_err}"), "overflow error");
}
