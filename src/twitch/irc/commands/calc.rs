use crate::twitch::irc::TwitchBotClient;
use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Constant(String),
    Plus,
    Minus,
    Multiply,
    Divide,
    Power,
    LeftParen,
    RightParen,
}

fn tokenize(input: &str, constants: &HashMap<String, f64>) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '0'..='9' | '.' => {
                let mut num = String::new();
                num.push(ch);
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_digit(10) || next_ch == '.' {
                        num.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Number(num.parse().map_err(|e| format!("Invalid number: {}", e))?));
            },
            'a'..='z' | 'A'..='Z' | 'π' | 'φ' | 'τ' => {
                let mut name = String::new();
                name.push(ch);
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == 'π' || next_ch == 'φ' || next_ch == 'τ' {
                        name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let constant_name = match name.as_str() {
                    "π" => "pi",
                    "φ" => "phi",
                    "τ" => "tau",
                    _ => &name,
                };
                if constants.contains_key(constant_name) {
                    tokens.push(Token::Constant(constant_name.to_string()));
                } else {
                    return Err(format!("Unknown constant: {}", name));
                }
            },
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Multiply),
            '/' => tokens.push(Token::Divide),
            '^' => tokens.push(Token::Power),
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ' ' => {},
            _ => return Err(format!("Invalid character: {}", ch)),
        }
    }
    Ok(tokens)
}

fn evaluate(tokens: &[Token], constants: &HashMap<String, f64>) -> Result<f64, String> {
    let mut output = Vec::new();
    let mut operators = Vec::new();

    for token in tokens.iter() {
        match token {
            Token::Number(n) => output.push(*n),
            Token::Constant(name) => {
                if let Some(&value) = constants.get(name) {
                    output.push(value);
                } else {
                    return Err(format!("Unknown constant: {}", name));
                }
            },
            Token::Plus | Token::Minus => {
                while let Some(op) = operators.last() {
                    if *op != Token::LeftParen {
                        apply_operator(&mut output, operators.pop().unwrap())?;
                    } else {
                        break;
                    }
                }
                operators.push(token.clone());
            },
            Token::Multiply | Token::Divide => {
                while let Some(op) = operators.last() {
                    if matches!(op, Token::Multiply | Token::Divide | Token::Power) {
                        apply_operator(&mut output, operators.pop().unwrap())?;
                    } else {
                        break;
                    }
                }
                operators.push(token.clone());
            },
            Token::Power => operators.push(token.clone()),
            Token::LeftParen => operators.push(token.clone()),
            Token::RightParen => {
                while let Some(op) = operators.pop() {
                    if op == Token::LeftParen {
                        break;
                    }
                    apply_operator(&mut output, op)?;
                }
            },
        }
    }

    while let Some(op) = operators.pop() {
        apply_operator(&mut output, op)?;
    }

    output.pop().ok_or_else(|| "Invalid expression".to_string())
}

fn apply_operator(output: &mut Vec<f64>, op: Token) -> Result<(), String> {
    let b = output.pop().ok_or("Invalid expression: not enough operands")?;
    let a = output.pop().ok_or("Invalid expression: not enough operands")?;
    let result = match op {
        Token::Plus => a + b,
        Token::Minus => a - b,
        Token::Multiply => a * b,
        Token::Divide => {
            if b == 0.0 {
                return Err("Division by zero".to_string());
            }
            a / b
        },
        Token::Power => a.powf(b),
        _ => return Err("Invalid operator".to_string()),
    };
    output.push(result);
    Ok(())
}

pub async fn handle_calc(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.is_empty() {
        client.send_message(channel, "Usage: !calc <expression>").await?;
        return Ok(());
    }

    let mut constants = HashMap::new();
    constants.insert("pi".to_string(), std::f64::consts::PI);
    constants.insert("e".to_string(), std::f64::consts::E);
    constants.insert("phi".to_string(), (1.0 + 5.0_f64.sqrt()) / 2.0);
    constants.insert("tau".to_string(), std::f64::consts::TAU);

    let expression = params.join(" ");

    match tokenize(&expression, &constants).and_then(|tokens| evaluate(&tokens, &constants)) {
        Ok(result) => {
            let response = format!("Result: {}", result);
            client.send_message(channel, &response).await?;
        },
        Err(e) => {
            let error_message = format!("Error: {}", e);
            client.send_message(channel, &error_message).await?;
        }
    }

    Ok(())
}