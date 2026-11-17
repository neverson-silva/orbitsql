use tedious_rs::token::stream::{StreamParseError, TokenStreamParser, TokenStreamState};

#[test]
fn failed_parser_rejects_feed_and_finish_without_processing() {
    let mut parser = TokenStreamParser::new("7_4", false);
    let failure = parser.feed(&[0xD1]).unwrap_err();
    assert_eq!(parser.state(), &TokenStreamState::Failed);
    assert_eq!(parser.buffered_len(), 1);

    assert_eq!(parser.feed(&[0x81]), Err(failure.clone()));
    assert_eq!(parser.buffered_len(), 1);
    assert_eq!(parser.finish(), Err(failure));
}

#[test]
fn successful_finish_rejects_feed_and_repeated_finish_deterministically() {
    let mut parser = TokenStreamParser::new("7_4", false);
    assert_eq!(parser.finish(), Ok(()));
    assert_eq!(parser.state(), &TokenStreamState::Finished);

    let expected = StreamParseError::Invalid("token stream is already finished".to_owned());
    assert_eq!(parser.feed(&[0x81]), Err(expected.clone()));
    assert_eq!(parser.finish(), Err(expected));
    assert_eq!(parser.buffered_len(), 0);
}

#[test]
fn finish_of_incomplete_input_becomes_terminal() {
    let mut parser = TokenStreamParser::new("7_4", false);
    assert!(parser.feed(&[0x81]).is_err());
    assert_eq!(parser.finish(), Err(StreamParseError::NotEnoughData));
    assert_eq!(parser.state(), &TokenStreamState::Failed);
    assert_eq!(parser.feed(&[0, 0]), Err(StreamParseError::NotEnoughData));
    assert_eq!(parser.finish(), Err(StreamParseError::NotEnoughData));
}

#[test]
fn incomplete_feed_remains_recoverable_before_finish() {
    let mut parser = TokenStreamParser::new("7_4", false);
    assert!(parser.feed(&[0x81]).is_err());
    assert_eq!(parser.state(), &TokenStreamState::Reading);
}
