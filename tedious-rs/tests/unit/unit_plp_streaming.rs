use tedious_rs::value_parser::{PlpStreamParser, PlpStreamState, read_plp_stream};

fn plp(total: u64, chunks: &[&[u8]]) -> Vec<u8> {
    let mut value = total.to_le_bytes().to_vec();
    for chunk in chunks {
        value.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        value.extend_from_slice(chunk);
    }
    value.extend_from_slice(&0_u32.to_le_bytes());
    value
}

#[test]
fn emits_each_complete_chunk_without_waiting_for_the_terminal_marker() {
    let wire = plp(6, &[b"abc", b"def"]);
    let mut parser = PlpStreamParser::new();
    let mut emitted = Vec::new();

    for byte in wire.iter().take(wire.len() - 4) {
        let result = parser.feed(std::slice::from_ref(byte)).unwrap();
        assert!(result.consumed <= 1);
        if let Some(chunk) = result.chunk {
            emitted.push(chunk);
        }
    }

    assert_eq!(emitted, vec![b"abc".to_vec(), b"def".to_vec()]);
    assert_eq!(parser.state(), &PlpStreamState::ReadingChunkLength);
    assert_eq!(parser.emitted_length(), 6);

    let result = parser.feed(&wire[wire.len() - 4..]).unwrap();
    assert_eq!(result.consumed, 4);
    assert!(result.chunk.is_none());
    assert_eq!(parser.state(), &PlpStreamState::Completed);
}

#[test]
fn pause_applies_backpressure_without_consuming_input_and_resume_retries_it() {
    let wire = plp(3, &[b"abc"]);
    let mut parser = PlpStreamParser::new();
    parser.pause();

    let paused = parser.feed(&wire).unwrap();
    assert_eq!(paused.consumed, 0);
    assert!(paused.chunk.is_none());
    assert!(parser.is_paused());
    assert_eq!(parser.state(), &PlpStreamState::ReadingTotalLength);

    parser.resume();
    let result = parser.feed(&wire).unwrap();
    assert_eq!(result.chunk, Some(b"abc".to_vec()));
    assert_eq!(result.consumed, 15);
    let terminal = parser.feed(&wire[result.consumed..]).unwrap();
    assert_eq!(terminal.consumed, 4);
    assert!(terminal.chunk.is_none());
    assert_eq!(parser.state(), &PlpStreamState::Completed);
}

#[test]
fn incomplete_input_becomes_a_terminal_error_on_finish() {
    let wire = plp(5, &[b"hello"]);
    let mut parser = PlpStreamParser::new();
    parser.feed(&wire[..wire.len() - 2]).unwrap();

    let first = parser.finish().unwrap_err();
    let second = parser.feed(&wire[wire.len() - 2..]).unwrap_err();
    assert_eq!(first, second);
    assert_eq!(parser.state(), &PlpStreamState::Failed);
}

#[test]
fn malformed_known_total_is_terminal_and_preserves_the_same_error() {
    let wire = plp(4, &[b"abc"]);
    let mut parser = PlpStreamParser::new();
    let first_result = parser.feed(&wire).unwrap();
    let first = parser.feed(&wire[first_result.consumed..]).unwrap_err();
    let second = parser.feed(&[]).unwrap_err();
    assert_eq!(first, second);
    assert_eq!(parser.state(), &PlpStreamState::Failed);
}

#[test]
fn null_plp_is_terminal_without_allocating_a_chunk() {
    let wire = u64::MAX.to_le_bytes();
    let mut parser = PlpStreamParser::new();
    let result = parser.feed(&wire).unwrap();
    assert_eq!(result.consumed, wire.len());
    assert!(result.chunk.is_none());
    assert_eq!(parser.state(), &PlpStreamState::Null);
    assert_eq!(parser.finish().unwrap().consumed, 0);
}

#[test]
fn legacy_reader_retains_materializing_behavior_on_top_of_incremental_parser() {
    let wire = plp(6, &[b"abc", b"def"]);
    let parsed = read_plp_stream(&wire, 0).unwrap();
    assert_eq!(parsed.value, Some(vec![b"abc".to_vec(), b"def".to_vec()]));
    assert_eq!(parsed.offset, wire.len());
}
