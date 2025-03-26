use std::mem;

#[derive(Debug, Default)]
struct Parser {
    ast: Ast,
    statement: Statement,
    current: Option<CondOrEffect>,
    current_block: Option<Block>,
    state: CallState,
}

#[derive(Debug)]
enum CondOrEffect {
    Cond(Vec<Block>),
    Effect(Vec<Block>),
}

impl CondOrEffect {
    fn add_block(&mut self, block: Block) {
        match self {
            CondOrEffect::Cond(x) => x.push(block),
            CondOrEffect::Effect(x) => x.push(block),
        }
    }
}

impl Parser {
    fn eat(&mut self, ch: &char) -> Result<(), String> {
        dbg!(ch);
        match ch {
            '{' => {
                if self.current.is_some() {
                    return Err("Condition inside of condition".to_owned());
                }
                self.current = Some(CondOrEffect::Cond(Vec::new()));
            }
            '}' => {
                self.next_block()?;
                match self.current.take() {
                    Some(CondOrEffect::Cond(x)) => {
                        self.statement.add_condition(x);
                    }
                    _ => return Err("Closing nonexistent condition".to_owned()),
                }
            }
            ',' => self.next_statement()?,
            '\t' | ' ' => self.next_block()?,
            '+' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.current_block = Some(Block::InfoPortion {
                    key: "".to_owned(),
                    inverted: false,
                })
            }
            '-' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.current_block = Some(Block::InfoPortion {
                    key: "".to_owned(),
                    inverted: true,
                })
            }
            '~' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.current_block = Some(Block::Chance { val: 0 })
            }
            '=' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.state = CallState::None;
                self.current_block = Some(Block::Call {
                    function: Default::default(),
                    args: Default::default(),
                    inverted: false,
                })
            }
            '!' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.state = CallState::None;
                self.current_block = Some(Block::Call {
                    function: Default::default(),
                    args: Default::default(),
                    inverted: true,
                })
            }
            '%' => {
                self.next_block()?;
                if let Some(x) = self.current.take() {
                    match x {
                        CondOrEffect::Cond(_) => {
                            return Err("Trying to Effect opened Condition".to_owned());
                        }
                        CondOrEffect::Effect(arr) => self.statement.add_effect(arr),
                    }

                    self.current = None;
                } else {
                    self.current = Some(CondOrEffect::Effect(Vec::new()));
                }
            }
            c => {
                match &mut self.current_block {
                    None => self
                        .statement
                        .out
                        .get_or_insert_default()
                        .push(c.to_owned()),
                    Some(x) => x.push_ch(ch.to_owned(), &mut self.state)?,
                };
            }
        }

        Ok(())
    }

    fn next_block(&mut self) -> Result<(), String> {
        dbg!(&mut self.current_block);
        if self.current_block.is_none() {
            return Ok(());
        }

        match &mut self.current {
            None => return Err("Block without context".to_owned()),
            Some(x) => x.add_block(self.current_block.take().unwrap()),
        }
        Ok(())
    }

    fn next_statement(&mut self) -> Result<(), String> {
        self.next_block()?;
        let statement = mem::take(&mut self.statement);
        self.ast.0.push(statement);
        Ok(())
    }

    fn finish(mut self) -> Result<Ast, String> {
        self.next_statement()?;
        Ok(self.ast)
    }
}

#[derive(Debug, PartialEq, Clone)]
enum CallState {
    None,
    Opened(String),
    Closed,
}

impl Default for CallState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, PartialEq)]
enum Block {
    InfoPortion {
        key: String,
        inverted: bool,
    },
    Call {
        function: String,
        args: Vec<String>,
        inverted: bool,
    },
    Chance {
        val: i32,
    },
}

impl Block {
    fn push_ch(&mut self, ch: char, state: &mut CallState) -> Result<(), String> {
        match self {
            Self::InfoPortion { key, .. } => key.push(ch),
            Self::Chance { val } => {
                *val *= 10;
                *val += ch.to_string().parse::<i32>().map_err(|e| e.to_string())?
            }
            Self::Call {
                function,
                args,
                inverted: _,
            } => {
                match (state.clone(), ch) {
                    (CallState::None, '(') => {
                        *state = CallState::Opened("".to_owned());
                    }
                    (CallState::None, x) => {
                        function.push(x);
                        *state = CallState::None;
                    }
                    (CallState::Opened(_), '(') => return Err("Call is already opened".to_owned()),
                    (CallState::Opened(x), ':') => {
                        args.push(x);
                        *state = CallState::Opened("".to_owned());
                    }
                    (CallState::Opened(x), ')') => {
                        args.push(x);
                        *state = CallState::Closed;
                    }
                    (CallState::Opened(mut x), y) => {
                        x.push(y);
                        *state = CallState::Opened(x);
                    }

                    (CallState::Closed, _) => return Err("Call is already closed".to_owned()),
                };
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq)]
struct Statement {
    condition: Option<Vec<Block>>,
    effects: Option<Vec<Block>>,
    out: Option<String>,
}

impl Statement {
    fn add_condition(&mut self, mut blocks: Vec<Block>) {
        self.condition.get_or_insert(Vec::new()).append(&mut blocks);
    }

    fn add_effect(&mut self, mut blocks: Vec<Block>) {
        self.effects.get_or_insert(Vec::new()).append(&mut blocks);
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Ast(Vec<Statement>);

impl Ast {
    pub fn from(src: &str) -> Result<Self, String> {
        let mut parser = Parser::default();
        for char in src.chars() {
            parser.eat(&char)?;
        }
        parser.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_value() {
        let result = Ast::from("Y").unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0.get(0).unwrap().condition.is_none(), true);
        assert_eq!(result.0.get(0).unwrap().out.as_ref().unwrap(), "Y");
    }

    #[test]
    fn two_values() {
        let result = Ast::from("X, Y").unwrap();
        assert_eq!(result.0.len(), 2);
        assert_eq!(result.0.get(0).unwrap().out.as_ref().unwrap(), "X");
        assert_eq!(result.0.get(1).unwrap().out.as_ref().unwrap(), "Y");
    }

    #[test]
    fn empty_condition() {
        let result = Ast::from("{} X").unwrap();
        assert_eq!(result.0.len(), 1);
        assert_eq!(result.0.get(0).unwrap().out.as_ref().unwrap(), "X");
        assert_eq!(result.0.get(0).unwrap().condition.is_some(), true);
        assert_eq!(
            result.0.get(0).unwrap().condition.as_ref().unwrap().len(),
            0
        );
    }

    #[test]
    fn info_condition() {
        let result = Ast::from("{+xy} X").unwrap();
        let conds = result.0.get(0).unwrap().condition.as_ref().unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(conds.get(0).is_some(), true);
        assert_eq!(
            conds.get(0),
            Some(&Block::InfoPortion {
                key: "xy".to_owned(),
                inverted: false
            })
        );
    }

    #[test]
    fn info_condition_neg() {
        let result = Ast::from("{-xy} X").unwrap();
        let conds = result.0.get(0).unwrap().condition.as_ref().unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(
            conds.get(0),
            Some(&Block::InfoPortion {
                key: "xy".to_owned(),
                inverted: true
            })
        );
    }

    #[test]
    fn probability() {
        let result = Ast::from("{~10} X").unwrap();

        let conds = result.0.get(0).unwrap().condition.as_ref().unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(conds.get(0), Some(&Block::Chance { val: 10 }));
    }

    #[test]
    fn simple_call() {
        let result = Ast::from("{=f} X").unwrap();

        let conds = result.0.get(0).unwrap().condition.as_ref().unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(
            conds.get(0),
            Some(&Block::Call {
                function: "f".to_owned(),
                args: Vec::new(),
                inverted: false,
            })
        );
    }

    #[test]
    fn complex() {
        let result = Ast::from("{=A(a1:a2) !B +C -D ~30} X %=E(e1) +F -G%, Y").unwrap();
        dbg!(&result);

        assert_eq!(
            result,
            Ast(vec![
                Statement {
                    condition: Some(vec![
                        Block::Call {
                            function: "A".to_owned(),
                            args: vec!["a1".to_owned(),"a2".to_owned()],
                            inverted: false,
                        },
                        Block::Call {
                            function: "B".to_owned(),
                            args: vec![],
                            inverted: true,
                        },
                        Block::InfoPortion {
                            key: "C".to_owned(),
                            inverted: false,
                        },
                        Block::InfoPortion {
                            key: "D".to_owned(),
                            inverted: true,
                        },
                        Block::Chance { val: 30 },
                    ],),
                    effects: Some(vec![
                        Block::Call {
                            function: "E".to_owned(),
                            args: vec!["e1".to_owned()],
                            inverted: false,
                        },
                        Block::InfoPortion {
                            key: "F".to_owned(),
                            inverted: false,
                        },
                        Block::InfoPortion {
                            key: "G".to_owned(),
                            inverted: true,
                        },
                    ],),
                    out: Some("X".to_owned(),),
                },
                Statement {
                    condition: None,
                    effects: None,
                    out: Some("Y".to_owned(),),
                },
            ],)
        )
    }
}
