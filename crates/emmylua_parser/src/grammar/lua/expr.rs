use crate::{
    grammar::ParseResult,
    kind::{BinaryOperator, LuaOpKind, LuaSyntaxKind, LuaTokenKind, UnaryOperator, UNARY_PRIORITY},
    parser::{LuaParser, MarkerEventContainer},
    parser_error::LuaParseError,
};

use super::{expect_token, if_token_bump, parse_block};

pub fn parse_expr(p: &mut LuaParser) -> ParseResult {
    return parse_sub_expr(p, 0);
}

fn parse_sub_expr(p: &mut LuaParser, limit: i32) -> ParseResult {
    let uop = LuaOpKind::to_unary_operator(p.current_token());
    let mut cm = if uop != UnaryOperator::OpNop {
        let m = p.mark(LuaSyntaxKind::UnaryExpr);
        let range = p.current_token_range();
        p.bump();
        match parse_sub_expr(p, UNARY_PRIORITY) {
            Ok(_) => {}
            Err(err) => {
                p.push_error(LuaParseError::from_source_range(
                    "unary operator not followed by expression",
                    range,
                ));
                return Err(err);
            }
        }
        m.complete(p)
    } else {
        parse_simple_expr(p)?
    };

    let mut bop = LuaOpKind::to_binary_operator(p.current_token());
    while bop != BinaryOperator::OpNop && bop.get_priority().left > limit {
        let range = p.current_token_range();
        let m = cm.precede(p, LuaSyntaxKind::BinaryExpr);
        p.bump();
        match parse_sub_expr(p, bop.get_priority().right) {
            Ok(_) => {}
            Err(err) => {
                p.push_error(LuaParseError::from_source_range(
                    "binary operator not followed by expression",
                    range,
                ));

                return Err(err);
            }
        }

        cm = m.complete(p);
        bop = LuaOpKind::to_binary_operator(p.current_token());
    }

    Ok(cm)
}

fn parse_simple_expr(p: &mut LuaParser) -> ParseResult {
    match p.current_token() {
        LuaTokenKind::TkInt
        | LuaTokenKind::TkFloat
        | LuaTokenKind::TkComplex
        | LuaTokenKind::TkNil
        | LuaTokenKind::TkTrue
        | LuaTokenKind::TkFalse
        | LuaTokenKind::TkDots
        | LuaTokenKind::TkString
        | LuaTokenKind::TkLongString => {
            let m = p.mark(LuaSyntaxKind::LiteralExpr);
            p.bump();
            Ok(m.complete(p))
        }
        LuaTokenKind::TkLeftBrace => parse_table_expr(p),
        LuaTokenKind::TkFunction => parse_closure_expr(p),
        _ => parse_suffixed_expr(p),
    }
}

pub fn parse_closure_expr(p: &mut LuaParser) -> ParseResult {
    let m = p.mark(LuaSyntaxKind::ClosureExpr);

    if_token_bump(p, LuaTokenKind::TkFunction);
    parse_param_list(p)?;

    if p.current_token() != LuaTokenKind::TkEnd {
        parse_block(p)?;
    }

    expect_token(p, LuaTokenKind::TkEnd)?;
    Ok(m.complete(p))
}

fn parse_param_list(p: &mut LuaParser) -> ParseResult {
    let m = p.mark(LuaSyntaxKind::ParamList);

    expect_token(p, LuaTokenKind::TkLeftParen)?;
    if p.current_token() != LuaTokenKind::TkRightParen {
        parse_param_name(p)?;
        while p.current_token() == LuaTokenKind::TkComma {
            p.bump();
            parse_param_name(p)?;
        }
    }

    expect_token(p, LuaTokenKind::TkRightParen)?;
    Ok(m.complete(p))
}

fn parse_param_name(p: &mut LuaParser) -> ParseResult {
    let m = p.mark(LuaSyntaxKind::ParamName);

    if p.current_token() == LuaTokenKind::TkName || p.current_token() == LuaTokenKind::TkDots {
        p.bump();
    } else {
        return Err(LuaParseError::from_source_range(
            "expect parameter name",
            p.current_token_range(),
        ));
    }

    Ok(m.complete(p))
}

fn parse_table_expr(p: &mut LuaParser) -> ParseResult {
    let m = p.mark(LuaSyntaxKind::TableExpr);
    p.bump();

    if p.current_token() == LuaTokenKind::TkRightBrace {
        p.bump();
        return Ok(m.complete(p));
    }

    parse_field(p)?;
    while p.current_token() == LuaTokenKind::TkComma
        || p.current_token() == LuaTokenKind::TkSemicolon
    {
        p.bump();
        if p.current_token() == LuaTokenKind::TkRightBrace {
            break;
        }
        parse_field(p)?;
    }

    expect_token(p, LuaTokenKind::TkRightBrace)?;
    Ok(m.complete(p))
}

fn parse_field(p: &mut LuaParser) -> ParseResult {
    let mut m = p.mark(LuaSyntaxKind::TableFieldValue);

    if p.current_token() == LuaTokenKind::TkLeftBracket {
        m.set_kind(p, LuaSyntaxKind::TableFieldAssign);
        p.bump();
        parse_expr(p)?;
        expect_token(p, LuaTokenKind::TkRightBracket)?;
        expect_token(p, LuaTokenKind::TkAssign)?;
        parse_expr(p)?;
    } else if p.current_token() == LuaTokenKind::TkName {
        if p.peek_next_token() == LuaTokenKind::TkAssign {
            m.set_kind(p, LuaSyntaxKind::TableFieldAssign);
            p.bump();
            p.bump();
            parse_expr(p)?;
        } else {
            parse_expr(p)?;
        }
    } else {
        parse_expr(p)?;
    }

    Ok(m.complete(p))
}

fn parse_suffixed_expr(p: &mut LuaParser) -> ParseResult {
    let mut cm = parse_primary_expr(p)?;
    loop {
        match p.current_token() {
            LuaTokenKind::TkDot | LuaTokenKind::TkColon | LuaTokenKind::TkLeftBracket => {
                let m = cm.precede(p, LuaSyntaxKind::IndexExpr);
                parse_index_struct(p)?;
                cm = m.complete(p);
            }
            LuaTokenKind::TkLeftParen
            | LuaTokenKind::TkLongString
            | LuaTokenKind::TkString
            | LuaTokenKind::TkLeftBrace => {
                let m = cm.precede(p, LuaSyntaxKind::CallExpr);
                parse_args(p)?;
                cm = m.complete(p);
            }
            _ => {
                return Ok(cm);
            }
        }
    }
}

fn parse_primary_expr(p: &mut LuaParser) -> ParseResult {
    match p.current_token() {
        LuaTokenKind::TkName => {
            let m = p.mark(LuaSyntaxKind::NameExpr);
            p.bump();
            Ok(m.complete(p))
        }
        LuaTokenKind::TkLeftParen => {
            let m = p.mark(LuaSyntaxKind::ParenExpr);
            p.bump();
            parse_expr(p)?;
            expect_token(p, LuaTokenKind::TkRightParen)?;
            Ok(m.complete(p))
        }
        _ => Err(LuaParseError::from_source_range(
            "expect primary expression",
            p.current_token_range(),
        )),
    }
}

fn parse_index_struct(p: &mut LuaParser) -> Result<(), LuaParseError> {
    match p.current_token() {
        LuaTokenKind::TkLeftBracket => {
            p.bump();
            parse_expr(p)?;
            expect_token(p, LuaTokenKind::TkRightBracket)?;
        }
        LuaTokenKind::TkDot | LuaTokenKind::TkColon => {
            p.bump();
            expect_token(p, LuaTokenKind::TkName)?;
        }
        _ => {
            return Err(LuaParseError::from_source_range(
                "expect index struct",
                p.current_token_range(),
            ));
        }
    }

    Ok(())
}

fn parse_args(p: &mut LuaParser) -> ParseResult {
    let m = p.mark(LuaSyntaxKind::CallArgList);
    match p.current_token() {
        LuaTokenKind::TkLeftParen => {
            p.bump();
            if p.current_token() != LuaTokenKind::TkRightParen {
                parse_expr(p)?;
                while p.current_token() == LuaTokenKind::TkComma {
                    p.bump();
                    parse_expr(p)?;
                }
            }
            expect_token(p, LuaTokenKind::TkRightParen)?;
        }
        LuaTokenKind::TkLeftBrace => {
            parse_table_expr(p)?;
        }
        LuaTokenKind::TkString | LuaTokenKind::TkLongString => {
            p.bump();
        }
        _ => {
            return Err(LuaParseError::from_source_range(
                "expect args",
                p.current_token_range(),
            ));
        }
    }

    Ok(m.complete(p))
}
