use crate::ast::ast::{Binary, Constant, Expr, RawExpr, RawPattern, RawType};
use crate::util::persistent::{adventure, Snapshot};
use std::collections::hash_set::Union;
/** Interpreting for the System F AST */
use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use super::ast::{Decl, Prog, Pattern, Ident, Type};
use super::error::TypeError;
use super::semant::{check_decl, check_expr};

/** Evaluates `expr` under `store` */
pub fn eval_expr(expr: &Expr, store: &mut Snapshot<Store>) -> Result<RawExpr, TypeError> {
    let mut ctxt = Snapshot::new(store.current().typ_store.clone());
    ctxt.enter();
    check_expr(&expr, &mut ctxt, &mut Snapshot::default())?;
    ctxt.exeunt();
    Ok(eval(store, expr))
}

/** Evaluates `decl` under current `store`, and add value to `store` */
pub fn eval_decl(decl: &Decl, store: &mut Snapshot<Store>) -> Result<(), TypeError> {
    let mut ctxt = Snapshot::new(store.current().typ_store.clone());
    ctxt.enter();
    check_decl(decl, &mut ctxt)?;
    ctxt.exeunt();
    let body = eval(store, &decl.body.expr);
    let curr = store.current();
    curr.val_store.insert(decl.id.clone(), body);
    curr.typ_store.insert(decl.id.clone(), decl.sig.typ.clone());
    Ok(())
}

/** Evaluates program */
pub fn eval_prog(prog: &Prog) -> Result<(), TypeError> {
    let mut store = Snapshot::default();
    for id in &prog.order {
        let decl = &prog.declarations[id];
        eval_decl(decl, &mut store)?;
    }
    Ok(())
}

pub fn eval_closed_expr(expr: &Expr) -> RawExpr {
    let mut store = Snapshot::default();
    eval_expr(expr, &mut store).unwrap()
}


// // Taken from https://github.com/jofas/map_macro
// macro_rules! set {
//     {$($v: expr),* $(,)?} => {
//         HashSet::from([$($v,)*])
//     };
// }

// /// Free variables
// fn fv(expr: &Expr) -> HashSet<String> {
//     use RawExpr::*;
//     let expr = &expr.expr;
//     match expr {
//         Con { .. } => set!(),
//         Var { id } => set!(id.clone()),
//         Let { pat, exp, body } => todo!(),
//         EApp { exp, arg } => fv(exp).union(&fv(arg)),
//         TApp { exp, arg } => todo!(),
//         Tuple { entries } => todo!(),
//         Binop { lhs, op, rhs } => todo!(),
//         Lambda { arg, body } => todo!(),
//         Any { arg, body } => todo!(),
//         If { cond, branch_t, branch_f } => todo!(),
//     }
// }

/** The evaluation function that returns the value of `expr` under the store `store`, while potentially updating `store` with new bindings. */
fn eval(store: &mut Snapshot<Store>, expr: &RawExpr) -> RawExpr {
    use RawExpr::*;
    // println!("-------------------------------");
    // println!("Store:\n{}", store);
    // println!("Evaluating: {}", expr);
    // println!("-------------------------------");
    let debug_temp_var = match &expr {
        // Constants being constants
        Con { val: _ } => expr.clone(),
        // Yeah
        Var { id } => match store.current().get_val(id) {
            Some(value) => value.clone(),
            None => panic!("{}", TYPE_ERR_MSG),
        },
        Let { pat, exp, body } => {
            let exp_neu = eval(store, exp);
            store.enter();
            bind_pat(&exp_neu, pat, store);
            let res = eval(store, body);
            store.exeunt();
            res
        }
        EApp { exp, arg } => {
            let func = eval(store, exp);
            let param = eval(store, arg);
            // lhs needs to be a value, which is a lambda expression by strength reduction
            let res = match func {
                Lambda {
                    arg: (var, typ),
                    body,
                } => {
                    // Update the store
                    let curr = store.current();
                    curr.val_store.insert(var.name.clone(), param);
                    curr.typ_store.insert(var.name, typ.typ);
                    eval(store, &body.expr)
                }
                _ => panic!("{}", TYPE_ERR_MSG),
            };
            res
        }
        // TODO properly apply
        TApp { exp, arg } => {
            if let Any { arg: t, body } = eval(store, exp) {
                store.current().typ_store.insert(t.name, arg.typ.clone());
                eval(store, &body.expr)
            } else {
                panic!("{}", TYPE_ERR_MSG)
            }
        }
        Tuple { entries } => {
            let neu = entries.iter().map(|e| Expr::new(eval(store, e))).collect();
            Tuple { entries: neu }
        }
        Binop { lhs, op, rhs } => {
            use Binary::*;
            use Constant::*;
            match op {
                // Integer arguments
                Add | Sub | Mul | Eq | Lt | Gt | Ne => {
                    let lhs_nf = eval(store, lhs);
                    let rhs_nf = eval(store, rhs);
                    if let (Con { val: Integer(l) }, Con { val: Integer(r) }) = (&lhs_nf, &rhs_nf) {
                        match op {
                            Add => Con {
                                val: Integer(l + r),
                            },
                            Sub => Con {
                                val: Integer(l - r),
                            },
                            Mul => Con {
                                val: Integer(l * r),
                            },
                            Eq => Con {
                                val: Boolean(l == r),
                            },
                            Lt => Con {
                                val: Boolean(l < r),
                            },
                            Gt => Con {
                                val: Boolean(l > r),
                            },
                            Ne => Con {
                                val: Boolean(l != r),
                            },
                            // Unreachable
                            _ => panic!(),
                        }
                    } else {
                        Binop {
                            lhs: Box::new(Expr::new(lhs_nf)),
                            op: op.clone(),
                            rhs: Box::new(Expr::new(rhs_nf)),
                        }
                    }
                }
                _ => {
                    let lhs_nf = eval(store, lhs);
                    let rhs_nf = eval(store, rhs);
                    if let (Con { val: Boolean(l) }, Con { val: Boolean(r) }) = (&lhs_nf, &rhs_nf) {
                        match op {
                            And => Con {
                                val: Boolean(l & r),
                            },
                            Or => Con {
                                val: Boolean(l | r),
                            },
                            _ => panic!(),
                        }
                    } else {
                        Binop {
                            lhs: Box::new(Expr::new(lhs_nf)),
                            op: op.clone(),
                            rhs: Box::new(Expr::new(rhs_nf)),
                        }
                    }
                }
            }
        }
        Lambda { .. } => expr.clone(),
        Any { .. } => expr.clone(),
        // {
        //     store.enter();
        //     store.current().typ_store.remove(&arg.name);
        //     let body_new = eval(store, body);
        //     store.exeunt();
        //     Any {
        //         arg: arg.clone(),
        //         body: Box::new(Expr::new(body_new)),
        //     }
        // }
        If {
            cond,
            branch_t,
            branch_f,
        } => {
            adventure!(cond_new, eval(store, cond), store);
            // If terminates, it should normalize to boolean constant
            if let Con {
                val: Constant::Boolean(b),
            } = cond_new
            {
                if b {
                    eval(store, branch_t)
                } else {
                    eval(store, branch_f)
                }
            } else {
                panic!("{}", TYPE_ERR_MSG)
            }
        }
    };
    debug_temp_var
}

/// Pattern matches `pat` recursively and binds to `exp`
fn bind_pat(exp: &RawExpr, pat: &RawPattern, store: &mut Snapshot<Store>) {
    match (exp, pat) {
        (RawExpr::Tuple { entries }, RawPattern::Tuple(patterns)) => {
            // Since we type check beforehand, these two vectors must have the same length
            for (e, p) in entries.iter().zip(patterns) {
                bind_pat(e, p, store)
            }
        }
        (_, RawPattern::Wildcard(_)) => (),
        (_, RawPattern::Binding(id, typ)) => {
            let value = eval(store, exp);
            let curr = store.current();
            curr.val_store.insert(id.to_string(), value);
            curr.typ_store.insert(id.to_string(), typ.typ.clone());
        }
        _ => panic!("{}", TYPE_ERR_MSG),
    }
}

// // Returns Some(ref), where `ref` is where the variable `v` occurs inside pattern `p`. None otherwise.
// fn find_binding<'a>(p: &'a mut Pattern, v: &'a str) -> Option<&'a mut String> {
//     match &mut p.pat {
// 	RawPattern::Wildcard(_) => None,
// 	RawPattern::Binding(u, _) => {
// 	    if u.name == v { Some(&mut u.name) }
// 	    else { None }
// 	}
// 	RawPattern::Tuple(pats) => {
// 	    for pat in pats {
// 		let cont = find_binding(pat, v);
// 		if cont.is_some() { return cont }
// 	    }
// 	    return None
// 	}
//     }
// }

impl RawPattern {

    /// Whether `self` contains the variable `var`
    fn contains_var(&self, var: &str) -> bool {
	self.any(|v, _| v.name == var)
    }

    /// Returns `true` iff all binding in `self` satisfies predicate `pred`
    fn all<F>(&self, pred: F) -> bool
    where F: Fn(&Ident, &Type) -> bool {
	match self {
	    RawPattern::Wildcard(_) => true,
	    RawPattern::Binding(v, t) => pred(v, t),
	    RawPattern::Tuple(pats) => pats.iter().all(|pat| pat.all(&pred)),
	}
    }

    /// Returns `true` iff any binding in `self` satisfies predicate `pred`
    fn any<F>(&self, pred: F) -> bool
    where F: Fn(&Ident, &Type) -> bool {
	match self {
	    RawPattern::Wildcard(_) => false,
	    RawPattern::Binding(v, t) => pred(v, t),
	    RawPattern::Tuple(pats) => pats.iter().any(|pat| pat.any(&pred)),
	}
    }
}

/// Returns `true` iff `var` is a free variable somewhere in `expression`
fn fv(var: &str, expression: &RawExpr) -> bool {
    use RawExpr::*;
    match expression {
        Con { .. } => false,
        Var { id } => id == var,
        Let { pat, exp, body } => {
	    fv(var, exp) |
	    (!&pat.contains_var(var) & fv(var, body))
	}
        EApp { exp, arg } => {
	    fv(var, exp) | fv(var, arg)
	}
        TApp { exp, .. } => fv(var, exp),
        Tuple { entries } => entries.iter().any(|e| fv(var, e)),
        Binop { lhs, op: _, rhs } => {
	    fv(var, lhs) | fv(var, rhs)
	}
        Lambda { arg, body } => {
	    (arg.0.name != var) & fv(var, body)
	}
        Any { arg: _, body } =>
	    fv(var, body),
        If { cond, branch_t, branch_f } => {
	    fv(var, cond) |
	    fv(var, branch_t) |
	    fv(var, branch_f)
	}
    }
}

// /** Performs capture-avoiding substitution
//     `expression`: The expression to perform substitute on
//     `var`: The variable to substitute
//     `val`: The value to sub for
//  */
// fn subst(expression: &mut RawExpr, var: &str, val: &RawExpr) {
//     use RawExpr::*;

//     match expression {
//         Con { .. } => (),
//         Var { id } => {
//             if id == var {
//                 *expression = val.clone();
//             } else {
//                 ()
//             }
//         },
// 	// Since `let x = e1 in e2` is syntactic sugar for `(\x. e2) e1` in STLC, we want to substitute e1, which is `exp` here.
//         Let { pat, exp, body } => {
// 	    subst(exp, var, val);
// 	    // Do nothing if same variable is bound in `pat`
// 	    if pat.contains_var(var) {
// 		()
// 	    } else {

// 	    }
// 	}
//         EApp { exp, arg } => {
// 	    subst(exp, var, val);
// 	    subst(arg, var, val)
// 	}
//         TApp { exp, .. } => {
// 	    subst(exp, var, val)
// 	}
//         Tuple { entries } => entries
//             .iter_mut()
//             .for_each(|e| subst(e, var, val)),
//         Binop { lhs, op: _, rhs } => {
//             subst(lhs, var, val);
//             subst(rhs, var, val)
//         }
//         Lambda { arg, body } => todo!(),
//         Any { arg: _, body } => {
// 	    subst(body, var, val)
// 	}
//         If {
//             cond,
//             branch_t,
//             branch_f,
//         } => {
// 	    subst(cond, var, val);
// 	    subst(branch_t, var, val);
// 	    subst(branch_f, var, val)
// 	}
//     }
// }

// fn substitute_type(exp: RawExpr, var: &str, typ: &RawType) -> RawExpr {
//     todo!()
// }

const TYPE_ERR_MSG: &str =
    "Type mismatch during interpretation. This shouldn't happen. Did you typecheck?";

/** A struct representing the "Store"s.
`val_store` is mapping from variable names to its (value, type) pair
`typ_store` maps type variable names to types */
#[derive(Clone, Default)]
pub struct Store {
    val_store: HashMap<String, RawExpr>,
    typ_store: HashMap<String, RawType>,
}

impl Display for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.val_store {
            let t = &self.typ_store[k];
            writeln!(f, "{k} : {t} := {v}")?
        }
        write!(f, "")
    }
}

impl Store {
    fn get_val(&self, key: &str) -> Option<&RawExpr> {
        self.val_store.get(key)
    }
}
