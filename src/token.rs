#[derive(PartialEq, Debug)]
pub enum Token {
    LT,
    GT,
    PLUS,
    MINUS,
    DOT,
    COMMA,
    LSQB,
    RSQB,
}

pub fn tokenize(codes: &str) -> Result<Vec<Token>, Box<dyn std::error::Error>> {
    let mut tokens = vec![];
    for c in codes.chars() {
        match c {
            '<' => tokens.push(Token::LT),
            '>' => tokens.push(Token::GT),
            '+' => tokens.push(Token::PLUS),
            '-' => tokens.push(Token::MINUS),
            '.' => tokens.push(Token::DOT),
            ',' => tokens.push(Token::COMMA),
            '[' => tokens.push(Token::LSQB),
            ']' => tokens.push(Token::RSQB),
            _ => (),
        }
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::*;
    #[test]
    fn tokenize_test() {
        let codes = " test \n>[-].,+<//;; 0;;\n";
        assert_eq!(
            vec![GT, LSQB, MINUS, RSQB, DOT, COMMA, PLUS, LT],
            tokenize(codes).unwrap()
        )
    }
}
