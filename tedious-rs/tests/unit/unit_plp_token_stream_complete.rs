use tedious_rs::token::stream::{StreamToken, TokenStreamParser, TokenStreamState};
use tedious_rs::value_parser::{PlpStreamParser, PlpStreamState};

fn plp(total: u64, chunks: &[&[u8]]) -> Vec<u8> {
    let mut wire = total.to_le_bytes().to_vec();
    for chunk in chunks {
        wire.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        wire.extend_from_slice(chunk);
    }
    wire.extend_from_slice(&0_u32.to_le_bytes());
    wire
}

fn done() -> Vec<u8> {
    vec![
        0xFD, 0x10, 0x00, 0x22, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

#[test]
fn token_stream_polls_one_token_after_packet_boundaries() {
    let input = done();
    let mut parser = TokenStreamParser::new("7_4", false);
    let mut observed = Vec::new();

    for byte in &input[..input.len() - 1] {
        assert!(
            parser
                .feed_next(std::slice::from_ref(byte))
                .unwrap()
                .is_none()
        );
    }
    observed.push(parser.feed_next(&input[input.len() - 1..]).unwrap());

    assert_eq!(observed.len(), 1);
    assert!(matches!(observed[0], Some(StreamToken::Control(_))));
    assert_eq!(parser.buffered_len(), 0);
    assert_eq!(parser.finish(), Ok(()));
    assert_eq!(parser.state(), &TokenStreamState::Finished);
}

#[test]
fn token_stream_pause_resume_preserves_buffered_bytes_for_polling() {
    let input = done();
    let mut parser = TokenStreamParser::new("7_4", false);
    parser.pause();
    assert!(parser.feed_next(&input).unwrap().is_none());
    assert_eq!(parser.buffered_len(), input.len());
    let tokens = parser.resume().unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(matches!(tokens[0], StreamToken::Control(_)));
}

#[test]
fn plp_emits_progressive_chunks_without_accumulating_the_value() {
    let input = plp(9, &[b"abc", b"defghi"]);
    let mut parser = PlpStreamParser::new();
    let mut chunks = Vec::new();
    let mut consumed_total = 0;

    for byte in &input {
        let output = parser.feed(std::slice::from_ref(byte)).unwrap();
        consumed_total += output.consumed;
        if let Some(chunk) = output.chunk {
            assert!(chunk.len() <= 6);
            chunks.push(chunk);
        }
    }

    assert_eq!(consumed_total, input.len());
    assert_eq!(chunks, vec![b"abc".to_vec(), b"defghi".to_vec()]);
    assert_eq!(parser.state(), &PlpStreamState::Completed);
    assert_eq!(parser.emitted_length(), 9);
}

#[test]
fn plp_pause_resume_does_not_consume_input() {
    let input = plp(3, &[b"abc"]);
    let mut parser = PlpStreamParser::new();
    parser.pause();
    let paused = parser.feed(&input).unwrap();
    assert_eq!(paused.consumed, 0);
    assert!(paused.chunk.is_none());
    parser.resume();
    let output = parser.feed(&input).unwrap();
    assert_eq!(output.chunk, Some(b"abc".to_vec()));
    assert_eq!(output.consumed, 15);
    let end = parser.feed(&input[output.consumed..]).unwrap();
    assert_eq!(end.consumed, 4);
    assert_eq!(parser.state(), &PlpStreamState::Completed);
}

#[test]
fn plp_null_is_terminal_and_known_length_mismatch_is_rejected() {
    let mut null_parser = PlpStreamParser::new();
    let null = u64::MAX.to_le_bytes();
    let null_output = null_parser.feed(&null).unwrap();
    assert_eq!(null_output.consumed, 8);
    assert_eq!(null_parser.state(), &PlpStreamState::Null);
    assert!(null_parser.feed(&[1]).unwrap().chunk.is_none());

    let mut mismatch_parser = PlpStreamParser::new();
    let mismatch = plp(4, &[b"abc"]);
    let first = mismatch_parser.feed(&mismatch).unwrap();
    let error = mismatch_parser
        .feed(&mismatch[first.consumed..])
        .unwrap_err();
    assert_eq!(mismatch_parser.state(), &PlpStreamState::Failed);
    assert_eq!(mismatch_parser.feed(&[]).unwrap_err(), error);
}

#[test]
fn plp_finish_turns_truncation_into_a_stable_terminal_error() {
    let input = plp(5, &[b"hello"]);
    let mut parser = PlpStreamParser::new();
    parser.feed(&input[..input.len() - 1]).unwrap();
    let error = parser.finish().unwrap_err();
    assert_eq!(parser.state(), &PlpStreamState::Failed);
    assert_eq!(parser.feed(&input[input.len() - 1..]).unwrap_err(), error);
}
