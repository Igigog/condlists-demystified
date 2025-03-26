use std::mem;

#[derive(Debug)]
struct Parser<'a> {
    ast: Ast<'a>,
    statement: Statement,
    current: Option<CondOrEffect>,
    current_block: Option<Block>,
    state: CallState,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            ast: Ast::empty(src),
            statement: Default::default(),
            current: Default::default(),
            current_block: Default::default(),
            state: Default::default(),
        }
    }
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

impl<'a> Parser<'a> {
    fn eat(&mut self, ch: &char, index: usize) -> Result<(), String> {
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
                    key: Slice::started_at(index + 1),
                    inverted: false,
                })
            }
            '-' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.current_block = Some(Block::InfoPortion {
                    key: Slice::started_at(index + 1),
                    inverted: true,
                })
            }
            '~' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.current_block = Some(Block::Chance {
                    val: Slice::started_at(index + 1),
                })
            }
            '=' => {
                if self.current_block.is_some() {
                    return Err("Starting started block".to_owned());
                }
                self.state = CallState::None;
                self.current_block = Some(Block::Call {
                    function: Slice::started_at(index + 1),
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
                    function: Slice::started_at(index + 1),
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
            _ => {
                match &mut self.current_block {
                    None => self
                        .statement
                        .out
                        .get_or_insert_with(|| Slice::started_at(index))
                        .push_ch(),
                    Some(x) => x.push_ch(ch.to_owned(), &mut self.state, index)?,
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
        self.ast.statements.push(statement);
        Ok(())
    }

    fn finish(mut self) -> Result<Ast<'a>, String> {
        self.next_statement()?;
        Ok(self.ast)
    }
}

#[derive(Debug, PartialEq)]
enum CallState {
    None,
    Opened(Slice),
    Closed,
}

impl Default for CallState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, PartialEq)]
struct Slice(usize, usize);

impl Slice {
    fn started_at(ix: usize) -> Self {
        Self(ix, 0)
    }

    fn push_ch(&mut self) {
        self.1 += 1;
    }
}

#[derive(Debug, PartialEq)]
enum Block {
    InfoPortion {
        key: Slice,
        inverted: bool,
    },
    Call {
        function: Slice,
        args: Vec<Slice>,
        inverted: bool,
    },
    Chance {
        val: Slice,
    },
}

impl Block {
    fn push_ch(&mut self, ch: char, state: &mut CallState, ix: usize) -> Result<(), String> {
        match self {
            Self::InfoPortion { key, .. } => key.push_ch(),
            Self::Chance { val } => {
                if !ch.is_ascii_digit() {
                    return Err("Not a digit!".to_owned());
                }
                val.push_ch();
            }
            Self::Call {
                function,
                args,
                inverted: _,
            } => {
                match (mem::replace(state, CallState::None), ch) {
                    (CallState::None, '(') => {
                        *state = CallState::Opened(Slice::started_at(ix));
                    }
                    (CallState::None, _) => {
                        function.push_ch();
                        *state = CallState::None;
                    }
                    (CallState::Opened(_), '(') => return Err("Call is already opened".to_owned()),
                    (CallState::Opened(x), ':') => {
                        args.push(x);
                        *state = CallState::Opened(Slice::started_at(ix));
                    }
                    (CallState::Opened(x), ')') => {
                        args.push(x);
                        *state = CallState::Closed;
                    }
                    (CallState::Opened(mut x), _) => {
                        x.push_ch();
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
    out: Option<Slice>,
}

impl Statement {
    fn add_condition(&mut self, mut blocks: Vec<Block>) {
        self.condition.get_or_insert(Vec::new()).append(&mut blocks);
    }

    fn add_effect(&mut self, mut blocks: Vec<Block>) {
        self.effects.get_or_insert(Vec::new()).append(&mut blocks);
    }
}

#[derive(Debug, PartialEq)]
pub struct Ast<'a> {
    orig: &'a str,
    statements: Vec<Statement>,
}

impl<'a> Ast<'a> {
    pub fn from(src: &'a str) -> Result<Self, String> {
        let mut parser = Parser::new(src);
        for (i, char) in src.chars().enumerate() {
            parser.eat(&char, i)?;
        }
        parser.finish()
    }

    fn empty(src: &'a str) -> Self {
        Self {
            orig: src,
            statements: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_value() {
        let result = Ast::from("Y").unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.statements.get(0).unwrap().condition.is_none(), true);
        assert_eq!(
            result.statements.get(0).unwrap().out.as_ref().unwrap(),
            &Slice(0, 1)
        );
    }

    #[test]
    fn two_values() {
        let result = Ast::from("X, Y").unwrap();
        assert_eq!(result.statements.len(), 2);
        assert_eq!(
            result.statements.get(0).unwrap().out.as_ref().unwrap(),
            &Slice(0, 1)
        );
        assert_eq!(
            result.statements.get(1).unwrap().out.as_ref().unwrap(),
            &Slice(3, 1)
        );
    }

    #[test]
    fn empty_condition() {
        let result = Ast::from("{} X").unwrap();
        assert_eq!(result.statements.len(), 1);
        assert_eq!(
            result.statements.get(0).unwrap().out.as_ref().unwrap(),
            &Slice(3, 1)
        );
        assert_eq!(result.statements.get(0).unwrap().condition.is_some(), true);
        assert_eq!(
            result
                .statements
                .get(0)
                .unwrap()
                .condition
                .as_ref()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn info_condition() {
        let result = Ast::from("{+xy} X").unwrap();
        let conds = result
            .statements
            .get(0)
            .unwrap()
            .condition
            .as_ref()
            .unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(conds.get(0).is_some(), true);
        assert_eq!(
            conds.get(0),
            Some(&Block::InfoPortion {
                key: Slice(2, 2),
                inverted: false
            })
        );
    }

    #[test]
    fn info_condition_neg() {
        let result = Ast::from("{-xy} X").unwrap();
        let conds = result
            .statements
            .get(0)
            .unwrap()
            .condition
            .as_ref()
            .unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(
            conds.get(0),
            Some(&Block::InfoPortion {
                key: Slice(2, 2),
                inverted: true
            })
        );
    }

    #[test]
    fn probability() {
        let result = Ast::from("{~10} X").unwrap();

        let conds = result
            .statements
            .get(0)
            .unwrap()
            .condition
            .as_ref()
            .unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(conds.get(0), Some(&Block::Chance { val: Slice(2, 2) }));
    }

    #[test]
    fn simple_call() {
        let result = Ast::from("{=f} X").unwrap();

        let conds = result
            .statements
            .get(0)
            .unwrap()
            .condition
            .as_ref()
            .unwrap();
        assert_eq!(conds.len(), 1);
        assert_eq!(
            conds.get(0),
            Some(&Block::Call {
                function: Slice(2, 1),
                args: Vec::new(),
                inverted: false,
            })
        );
    }

    #[test]
    fn complex() {
        let src = "{=A(a1:a2) !B +C -D ~30} X %=E(e1) +F -G%, Y";
        let result = Ast::from(src).unwrap();

        assert_eq!(
            result,
            Ast {
                orig: &src,
                statements: vec![
                    Statement {
                        condition: Some(vec![
                            Block::Call {
                                function: Slice(2, 1,),
                                args: vec![Slice(3, 2,), Slice(6, 2,),],
                                inverted: false,
                            },
                            Block::Call {
                                function: Slice(12, 1,),
                                args: vec![],
                                inverted: true,
                            },
                            Block::InfoPortion {
                                key: Slice(15, 1,),
                                inverted: false,
                            },
                            Block::InfoPortion {
                                key: Slice(18, 1,),
                                inverted: true,
                            },
                            Block::Chance { val: Slice(21, 2,) },
                        ],),
                        effects: Some(vec![
                            Block::Call {
                                function: Slice(29, 1,),
                                args: vec![Slice(30, 2,),],
                                inverted: false,
                            },
                            Block::InfoPortion {
                                key: Slice(36, 1,),
                                inverted: false,
                            },
                            Block::InfoPortion {
                                key: Slice(39, 1,),
                                inverted: true,
                            },
                        ],),
                        out: Some(Slice(25, 1,),),
                    },
                    Statement {
                        condition: None,
                        effects: None,
                        out: Some(Slice(43, 1,),),
                    },
                ],
            }
        )
    }
}
