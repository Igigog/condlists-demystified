use crate::parser::{Ast, Block, Condition, Effect, Slice};

#[derive(Debug, PartialEq)]
enum Tag {
    Statement,
    Block,
    Effect,
    Effects,
    Condition,
    Conditions,
    Output,
}

#[derive(Debug, Default, PartialEq)]
struct Metadata(Vec<(Slice, Tag)>);

struct IndentStr(String);

impl IndentStr {
    fn new() -> Self {
        Self(String::new())
    }

    fn push_str(&mut self, str: &str, indent: usize) {
        self.0.push_str(&" ".repeat(indent));
        self.0.push_str(str);
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

trait IntoLua {
    fn to_lua(&self, ast: &Ast, ix: usize, indent: usize) -> (String, Metadata);
}

impl IntoLua for Ast<'_> {
    fn to_lua(&self, _: &Ast, ix: usize, indent: usize) -> (String, Metadata) {
        let mut out = IndentStr::new();
        let mut metadata = Metadata::default();

        for statement in self.statements() {
            let has_conds = statement
                .conditions()
                .is_some_and(|x| !x.blocks().is_empty());
            if let Some(x) = statement.conditions() {
                if !x.blocks().is_empty() {
                    let mut lua_conds = IndentStr("if ".to_owned());
                    let (lua_val, meta) =
                        x.to_lua(self, ix + lua_conds.0.len() + out.0.len(), indent);
                    lua_conds.push_str(&lua_val, indent);
                    lua_conds.push_str("then\n", indent);

                    metadata.0.extend(meta.0);
                    out.push_str(&lua_conds.0, indent);
                }
            }
            if let Some(eff) = statement.effects() {
                let (lua_val, meta) = eff.to_lua(
                    self,
                    ix + out.0.len(),
                    indent + if has_conds { 4 } else { 0 },
                );

                meta.0.into_iter().for_each(|e| metadata.0.push(e));

                metadata
                    .0
                    .push((Slice::new(out.0.len(), lua_val.len()), Tag::Effects));
                out.push_str(&lua_val, indent);
            }

            let val = if let Some(x) = statement.val() {
                "\"".to_owned() + self.slice_as_str(x) + "\""
            } else {
                "nil".to_owned()
            };
            let lua_val = format!("return {}", val);

            metadata
                .0
                .push((Slice::new(out.0.len(), lua_val.len()), Tag::Output));
            out.push_str(&lua_val, indent + if has_conds { 4 } else { 0 });

            if has_conds {
                out.push_str("\nend\n", indent);
            }
            out.push_str("\n", indent);
        }

        metadata
            .0
            .push((Slice::new(ix, out.0.len()), Tag::Statement));
        (out.0, metadata)
    }
}

impl IntoLua for Effect {
    fn to_lua(&self, ast: &Ast, ix: usize, indent: usize) -> (String, Metadata) {
        let out = self
            .blocks()
            .iter()
            .map(|block| match block {
                Block::InfoPortion { key, inverted } => {
                    let start = (if *inverted {
                        "db.actor:disable_info_portion"
                    } else {
                        "db.actor:give_info_portion"
                    })
                    .to_owned();
                    format!("{}(\"{}\")\n", start, ast.slice_as_str(key))
                }
                Block::Call {
                    function,
                    args,
                    inverted: _,
                } => {
                    format!(
                        "xr_effects.{}({})\n",
                        ast.slice_as_str(function),
                        args.iter()
                            .map(|ar| format!("\"{}\"", ast.slice_as_str(ar)))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
                Block::Chance { val } => {
                    format!("math.random(1, 100) > {}\n", ast.slice_as_str(val))
                }
            })
            .fold((IndentStr::new(), Metadata::default()), |mut acc, b| {
                acc.1
                    .0
                    .push((Slice::new(ix + acc.0.0.len(), b.len()), Tag::Condition));
                acc.0.push_str(&b, indent);
                acc
            });
        (out.0.0, out.1)
    }
}

impl IntoLua for Condition {
    fn to_lua(&self, ast: &Ast, ix: usize, indent: usize) -> (String, Metadata) {
        let out = self
            .blocks()
            .iter()
            .map(|block| match block {
                Block::InfoPortion { key, inverted } => {
                    let start = (if *inverted {
                        "not db.actor:has_info"
                    } else {
                        "db.actor:has_info"
                    })
                    .to_owned();
                    format!("{}(\"{}\")\n", start, ast.slice_as_str(key))
                }
                Block::Call {
                    function,
                    args,
                    inverted,
                } => {
                    let mut lua_val = format!(
                        "xr_conditions.{}({})\n",
                        ast.slice_as_str(function),
                        args.iter()
                            .map(|ar| format!("\"{}\"", ast.slice_as_str(ar)))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    if *inverted {
                        lua_val = format!("not {}", lua_val);
                    }
                    lua_val
                }
                Block::Chance { val } => {
                    format!("math.random(1, 100) > {}\n", ast.slice_as_str(val))
                }
            })
            .fold((IndentStr::new(), Metadata::default()), |mut acc, b| {
                if !acc.0.is_empty() {
                    acc.0.push_str("and ", indent + 8);
                };
                acc.1
                    .0
                    .push((Slice::new(ix + acc.0.0.len(), b.len()), Tag::Condition));
                acc.0.0.push_str(&b);
                acc
            });
        (out.0.0, out.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_value() {
        let ast = Ast::from("Y").unwrap();
        assert_eq!(
            ast.to_lua(&ast, 0, 0),
            (
                "return \"Y\"".to_owned(),
                Metadata(vec![((Slice::new(0, 10)), Tag::Output)])
            )
        )
    }

    #[test]
    fn effect_call() {
        let ast = Ast::from("Y %=A%").unwrap();
        let val = ast.to_lua(&ast, 0, 0);
        dbg!(&val.0);
        assert_eq!(
            val,
            (
                "xr_effects.A()\n\
                return \"Y\""
                    .to_owned(),
                Metadata(vec![
                    ((Slice::new(0, 15)), Tag::Effect),
                    ((Slice::new(0, 15)), Tag::Effects),
                    ((Slice::new(15, 10)), Tag::Output)
                ])
            )
        )
    }

    #[test]
    fn complex() {
        let src = "{=A(a1:a2) !B +C -D ~30} X %=E(e1) +F -G%, Y";
        let ast = Ast::from(src).unwrap();

        let (lua, meta) = ast.to_lua(&ast, 0, 0);
        println!("{}", lua);
    }

    #[test]
    fn complex2() {
        let src = "{=A(a1:a2) !B +C -D ~30} X %=E(e1) +F -G%, {=A(a1:a2) !B +C -D ~30} Y, B";
        let ast = Ast::from(src).unwrap();

        let (lua, meta) = ast.to_lua(&ast, 0, 0);
        println!("{}", lua);
    }
}
