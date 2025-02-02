use super::item::*;
// use crate::comment::{CommentExtrator, DocumentComments};
use crate::item;
use crate::utils::GetPosAndLength;
use cranelift_isle::files::Files;
use cranelift_isle::{ast::*, error::Errors, lexer::*};
use lsp_types::Position;
use lsp_types::Range;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

/// All the information of a project.
pub struct Project {
    pub files: Arc<Files>,
    pub(crate) definitions: Vec<Def>,
    // pub(crate) defs: Defs,
    pub(crate) context: VisitContext,
    // pub(crate) comments: HashMap<PathBuf, DocumentComments>,
}

impl Project {
    pub fn empty() -> Self {
        Self {
            // defs: Defs {
            //     defs: Default::default(),
            //     filenames: Default::default(),
            //     file_texts: Default::default(),
            // },
            definitions: Default::default(),
            files: Default::default(),

            context: Default::default(),
            // comments: Default::default(),
        }
    }
    // pub fn get_filenames(&self) -> &Vec<Arc<str>> {
    //     &self.files.file_names
    // }
    #[allow(dead_code)]
    pub fn from_walk() -> Result<Self, cranelift_isle::error::Errors> {
        let mut files = Vec::new();
        for x in walkdir::WalkDir::new(std::env::current_dir().unwrap()) {
            let x = match x {
                Ok(x) => x,
                Err(_) => {
                    continue;
                }
            };
            if x.file_type().is_file() && x.file_name().to_str().unwrap().ends_with(".isle") {
                files.push(x.path().to_path_buf());
            }
        }
        Self::new(files)
    }

    pub fn get_filenames(&self) -> Vec<PathBuf> {
        self.files
            .file_names
            .iter()
            .map(|x| PathBuf::from_str(x.as_ref()).unwrap())
            .collect()
    }

    // fn get_comment(content: &str, defs: impl AstProvider) -> DocumentComments {
    //     let e = CommentExtrator::new(content);
    //     let mut poes = Vec::new();
    //     defs.with_def(|x| {
    //         if let Some(pos) = get_decl_pos(x) {
    //             poes.push(pos.clone());
    //             match x {
    //                 Def::Type(ty) => match &ty.ty {
    //                     TypeValue::Primitive(_, _) => {}
    //                     TypeValue::Enum(vs, _) => {
    //                         for v in vs.iter() {
    //                             poes.push(v.name.1);
    //                             for f in v.fields.iter() {
    //                                 poes.push(f.name.1);
    //                             }
    //                         }
    //                     }
    //                 },
    //                 _ => {}
    //             }
    //         }
    //     });
    //     DocumentComments::new(&e, &poes)
    // }

    pub fn new(
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, cranelift_isle::error::Errors> {
        let files = match Files::from_paths(paths) {
            Ok(files) => files,
            Err((path, err)) => {
                return Err(Errors::from_io(
                    err,
                    format!("cannot read file {}", path.display()),
                ))
            }
        };
        let files = Arc::new(files);
        let definitions = crate::defs_of_files(files.as_ref());
        let project = Self {
            files,
            definitions,
            context: VisitContext::new(),
            // comments,
        };

        // let mut comments = HashMap::new();
        // for (index, f) in files.iter().enumerate() {
        //     let e = Self::get_comment(
        //         project.file_content(index),
        //         project.get_vec_def_ast_provider_from_file_index(index),
        //     );
        //     comments.insert(f.clone(), e);
        // }
        // project.comments = comments;
        let mut dummy = DummyHandler {};
        project.run_full_visitor(&mut dummy);
        Ok(project)
    }

    fn file_content(&self, file_index: usize) -> &str {
        self.files
            .file_text(file_index)
            .map(|x| x.as_ref())
            .unwrap_or("")
    }

    pub fn run_visitor_for_file(&self, p: &PathBuf, handler: &mut dyn ItemOrAccessHandler) {
        let provider = match self.found_file_defs(p) {
            Some(x) => x,
            None => {
                log::error!("not found defs.");
                return;
            }
        };
        self.visit(provider, handler);
    }
    pub(crate) fn found_file_defs<'a>(&'a self, p: &PathBuf) -> Option<VecDefAstProvider<'a>> {
        let file_index = match self.found_file_index(p) {
            Some(x) => x,
            None => {
                log::error!("file index out found,{:?}", p);
                return None;
            }
        };
        return Some(self.get_vec_def_ast_provider_from_file_index(file_index));
    }

    fn get_vec_def_ast_provider_from_file_index<'a>(
        &'a self,
        file_index: usize,
    ) -> VecDefAstProvider<'a> {
        let mut ret = Vec::new();
        self.definitions.iter().for_each(|x| {
            if get_decl_pos(x)
                .map(|p| p.file == file_index)
                .unwrap_or(false)
            {
                ret.push(x);
            }
        });
        VecDefAstProvider::new(ret)
    }

    fn found_file_index(&self, p: &PathBuf) -> Option<usize> {
        for (index, x) in self.files.file_names.iter().enumerate() {
            if p.to_str().unwrap() == x {
                return Some(index);
            }
        }
        None
    }

    pub(crate) fn file_index_path(&self, index: usize) -> Option<PathBuf> {
        self.files
            .file_names
            .get(index)
            .map(|x| PathBuf::from_str(x.as_ref()).unwrap())
    }

    pub fn run_full_visitor(&self, handler: &mut dyn ItemOrAccessHandler) {
        let provider = ProjectAstProvider::new(self);
        self.visit(provider, handler);
    }

    pub(crate) fn mk_file_paths(&self) -> Vec<PathBuf> {
        self.files
            .file_names
            .iter()
            .map(|x| PathBuf::from_str(x.as_ref()).unwrap())
            .collect()
    }

    pub fn update_defs(&mut self, p: &PathBuf, content: &str) -> Result<(), Errors> {
        let file_index = match self.found_file_index(p) {
            Some(x) => x,
            None => {
                log::error!("old defs not found for {:?}", p.as_path());
                return std::result::Result::Ok(());
            }
        };
        let files: Vec<_> = self
            .mk_file_paths()
            .iter()
            .map(|x| {
                (
                    x.clone().to_str().unwrap().to_string(),
                    // only update the specified one from the parameter.
                    if x == p {
                        content.to_string()
                    } else {
                        "".to_string()
                    },
                )
            })
            .collect();
        let files = Files::from_names_and_contents(files);
        let defs = crate::defs_of_files(&files);

        // insert into `defs`.
        let mut slots = Vec::new();
        // delete all old `Def`
        self.definitions
            .iter_mut()
            .enumerate()
            .for_each(|(index, x)| {
                if let Some(pos) = get_decl_pos(x) {
                    if pos.file == file_index {
                        slots.push(index);
                    }
                }
            });
        let mut slots = &slots[..];
        for d in defs.into_iter() {
            if slots.len() > 0 {
                let index = slots[0];
                self.definitions[index] = d;
                slots = &slots[1..];
            } else {
                self.definitions.push(d);
            }
        }

        for s in slots {
            self.definitions[*s] = FALSE_DEF.clone();
        }

        self.context.delete_old_defs(file_index);

        let mut dummy = DummyHandler {};
        self.run_visitor_for_file(p, &mut dummy);

        // update comment
        // self.comments.insert(
        //     p.clone(),
        //     Self::get_comment(
        //         content,
        //         self.get_vec_def_ast_provider_from_file_index(file_index),
        //     ),
        // );

        std::result::Result::Ok(())
    }
}

pub(crate) fn get_decl_pos(d: &Def) -> Option<&Pos> {
    match d {
        Def::Pragma(_x) => None,
        Def::Type(x) => Some(&x.pos),
        Def::Rule(x) => Some(&x.pos),
        Def::Extractor(x) => Some(&x.pos),
        Def::Decl(x) => Some(&x.pos),
        Def::Extern(x) => Some(match x {
            Extern::Extractor {
                term,
                func: _,
                pos: _,
                infallible: _,
            } => &term.1,
            Extern::Constructor {
                term,
                func: _,
                pos: _,
            } => &term.1,
            Extern::Const {
                name,
                ty: _,
                pos: _,
            } => &name.1,
        }),
        Def::Converter(x) => Some(&x.pos),

        // ISLE language grammar verification extensions
        // out of scope for now
        Def::Spec(_) => todo!(),
        Def::Model(_) => todo!(),
        Def::Form(_) => todo!(),
        Def::Instantiation(_) => todo!(),
    }
}

impl Project {
    pub(crate) fn mk_location<T: GetPosAndLength>(&self, x: &T) -> Option<lsp_types::Location> {
        let (pos, length) = x.get_pos_and_len();
        let linemap = self.files.file_line_map(pos.file).unwrap();
        let line = linemap.line(pos.offset);
        let col = pos.offset - linemap.get(line).unwrap();

        let line = line as u32;
        let col = col as u32;

        self.files.file_names.get(pos.file).map(|x| {
            let s = x.clone();
            lsp_types::Location {
                uri: url::Url::from_file_path(
                    PathBuf::from_str(s.as_str()).unwrap(), //
                )
                .unwrap(),
                range: Range {
                    start: Position {
                        line,
                        character: col,
                    },
                    end: Position {
                        line,
                        character: col + length,
                    },
                },
            }
        })
    }
}

pub(crate) struct VisitContext {
    pub(crate) scopes: Rc<RefCell<Vec<Scope>>>,
}

impl Default for VisitContext {
    fn default() -> Self {
        let x = Self {
            scopes: Rc::new(RefCell::new(vec![Scope::new()])),
        };
        x
    }
}

#[derive(Default, Clone)]
pub(crate) struct Scope {
    pub(crate) items: HashMap<String, Item>,
}

impl Scope {
    fn new() -> Self {
        Self::default()
    }
}

impl VisitContext {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn delete_old_defs(&self, file_index: usize) {
        let mut keys = HashSet::new();
        for (k, v) in self.scopes.as_ref().borrow().first().unwrap().items.iter() {
            if v.def_file() == file_index {
                keys.insert(k.clone());
            }
        }
        keys.iter().for_each(|x| {
            self.scopes
                .as_ref()
                .borrow_mut()
                .first_mut()
                .unwrap()
                .items
                .remove(x);
        });
    }
    pub(crate) fn enter_item(&self, name: String, item: impl Into<Item>) {
        if name.as_str() == "_" {
            return;
        }

        let item: Item = item.into();
        let _pos = item.def_loc();
        // log::trace!(
        //     "enter item name:{} pos:{}:{} {}",
        //     name.as_str(),
        //     item.def_loc().0.line,
        //     item.def_loc().0.col,
        //     item
        // );
        self.scopes
            .as_ref()
            .borrow_mut()
            .last_mut()
            .unwrap()
            .items
            .insert(name, item);
    }

    pub(crate) fn query_item<R>(
        &self,
        name: &String,
        mut call_back: impl FnMut(&Item) -> R,
    ) -> Option<R> {
        for s in self.scopes.as_ref().borrow().iter().rev() {
            let t = s.items.get(name);
            if t.is_some() {
                return Some(call_back(t.unwrap()));
            }
        }
        None
    }
    pub(crate) fn query_item_clone(&self, name: &String) -> Item {
        self.query_item(name, |x| x.clone()).unwrap_or_default()
    }

    pub(crate) fn query_const<R>(
        &self,
        name: &String,
        mut call_back: impl FnMut(&Item) -> R,
    ) -> Option<R> {
        if let Some(x) = self
            .scopes
            .as_ref()
            .borrow()
            .first()
            .unwrap()
            .items
            .get(name)
            .map(|x| match x {
                Item::Const { .. } => Some(x),
                _ => None,
            })
            .flatten()
        {
            return Some(call_back(x));
        }

        None
    }
    pub(crate) fn query_const_clone(&self, name: &String) -> Item {
        self.query_const(name, |x| x.clone()).unwrap_or_default()
    }

    pub(crate) fn fix_decl_type(&self, name: &String, decl_ty: u8) {
        let s = || -> Option<()> {
            match self
                .scopes
                .as_ref()
                .borrow_mut()
                .first_mut()
                .unwrap()
                .items
                .get_mut(name)?
            {
                Item::Decl { decl: _, kind: ty } => {
                    ty.0 = ty.0 | decl_ty;
                }
                _ => {}
            };
            None
        };
        s();
    }
    pub(crate) fn enter_scope(&self, mut x: impl FnMut()) {
        let _guard = ScopesGuarder::new(self);
        self.scopes.as_ref().borrow_mut().push(Scope::new());
        x()
    }
}

pub trait ItemOrAccessHandler {
    /// Handle this item.
    fn handle_item_or_access(&mut self, p: &Project, _item: &ItemOrAccess);

    fn visit_body(&self) -> bool;

    /// Visitor should finished.
    fn finished(&self) -> bool;
}

lazy_static! {
    static ref FALSE_DEF: Def = Def::Type(Type {
        name: Ident("".to_string(), item::UNKNOWN_POS),
        is_extern: false,
        is_nodebug: false,
        ty: TypeValue::Primitive(Ident("".to_string(), item::UNKNOWN_POS), item::UNKNOWN_POS),
        pos: item::UNKNOWN_POS,
    });
}

pub trait AstProvider: Clone {
    fn with_def(&self, call_back: impl FnMut(&Def));
    fn with_pragma(&self, mut call_back: impl FnMut(&Pragma)) {
        self.with_def(|x| match x {
            Def::Pragma(x) => call_back(x),
            _ => {}
        })
    }
    fn with_type(&self, mut call_back: impl FnMut(&Type)) {
        self.with_def(|x| match x {
            Def::Type(x) => call_back(x),
            _ => {}
        })
    }
    fn with_rule(&self, mut call_back: impl FnMut(&Rule)) {
        self.with_def(|x| match x {
            Def::Rule(x) => call_back(x),
            _ => {}
        })
    }
    fn with_extractor(&self, mut call_back: impl FnMut(&Extractor)) {
        self.with_def(|x| match x {
            Def::Extractor(x) => call_back(x),
            _ => {}
        })
    }
    fn with_decl(&self, mut call_back: impl FnMut(&Decl)) {
        self.with_def(|x| match x {
            Def::Decl(x) => call_back(x),
            _ => {}
        })
    }
    fn with_extern(&self, mut call_back: impl FnMut(&Extern)) {
        self.with_def(|x| match x {
            Def::Extern(x) => call_back(x),
            _ => {}
        })
    }
    fn with_converter(&self, mut call_back: impl FnMut(&Converter)) {
        self.with_def(|x| match x {
            Def::Converter(x) => call_back(x),
            _ => {}
        })
    }
}

#[derive(Clone)]
struct ProjectAstProvider<'a> {
    p: &'a Project,
}

impl<'a> ProjectAstProvider<'a> {
    fn new(p: &'a Project) -> Self {
        Self { p }
    }
}
macro_rules! call_decl {
    ($x:expr,$call_back:ident) => {{
        if get_decl_pos($x)
            .map(|x| x.file != UNKNOWN_POS.file)
            .unwrap_or(false)
        {
            $call_back($x);
        }
    }};
}

impl<'a> AstProvider for ProjectAstProvider<'a> {
    fn with_def(&self, mut call_back: impl FnMut(&Def)) {
        self.p.definitions.iter().for_each(|x| {
            call_decl!(x, call_back);
        });
    }
}

#[derive(Clone)]
pub(crate) struct VecDefAstProvider<'a> {
    defs: Vec<&'a Def>,
}

impl<'a> VecDefAstProvider<'a> {
    pub(crate) fn new(defs: Vec<&'a Def>) -> Self {
        Self { defs }
    }
}

impl<'a> AstProvider for VecDefAstProvider<'a> {
    fn with_def(&self, mut call_back: impl FnMut(&Def)) {
        self.defs.iter().for_each(|x| {
            call_decl!(x, call_back);
        })
    }
}

#[derive(Clone)]
pub(crate) struct RefVecDefAstProvider<'a> {
    pub(crate) defs: &'a Vec<Def>,
}

impl<'a> AstProvider for RefVecDefAstProvider<'a> {
    fn with_def(&self, mut call_back: impl FnMut(&Def)) {
        self.defs.iter().for_each(|x| {
            call_decl!(x, call_back);
        })
    }
}

struct DummyHandler {}

impl ItemOrAccessHandler for DummyHandler {
    fn visit_body(&self) -> bool {
        false
    }
    fn finished(&self) -> bool {
        false
    }
    fn handle_item_or_access(&mut self, _p: &Project, _item: &ItemOrAccess) {}
}

pub(crate) fn get_rule_target(p: &Pattern) -> Option<(&String, Pos)> {
    match p {
        Pattern::Var { var: _, pos: _ } => None,
        Pattern::BindPattern {
            var,
            subpat: _,
            pos: _,
        } => Some((&var.0, var.1)),
        Pattern::Term {
            sym,
            args: _,
            pos: _,
        } => Some((&sym.0, sym.1)),
        _ => None,
    }
}

/// RAII type pop on when enter a scope.
#[must_use]
pub(crate) struct ScopesGuarder(Rc<RefCell<Vec<Scope>>>);

impl ScopesGuarder {
    pub(crate) fn new(s: &VisitContext) -> Self {
        Self(s.scopes.clone())
    }
}

impl Drop for ScopesGuarder {
    fn drop(&mut self) {
        self.0.as_ref().borrow_mut().pop().unwrap();
    }
}

// #[derive(PartialEq, Eq, Debug, Clone)]
// pub struct SymbolAndPos {
//     pub symbol: String,
//     pub pos: Pos,
// }

// impl Into<Ident> for SymbolAndPos {
//     fn into(self) -> Ident {
//         let pos = self.pos;
//         Ident(self.symbol, pos)
//     }
// }

// /// `ISLE` lexer compsite xxx and yyy together
// /// like xxx.yyy
// /// not a seperate token
// /// but one token `xxx.yyy`
// #[derive(PartialEq, Eq, Debug, Clone)]
// pub(crate) enum SplitedSymbol {
//     One(SymbolAndPos),
//     Two([SymbolAndPos; 2]),
// }

// impl From<&Ident> for SplitedSymbol {
//     fn from(value: &Ident) -> Self {
//         Self::from(&(value.0.clone(), value.1))
//     }
// }
// impl From<&(String, Pos)> for SplitedSymbol {
//     fn from(value: &(String, Pos)) -> Self {
//         let (s, pos) = value;
//         let mut index = None;
//         for (i, s) in s.as_bytes().iter().enumerate() {
//             if *s == 46
//             // ascii for  '.'
//             {
//                 index = Some(i);
//             }
//         }
//         match index {
//             Some(index) => {
//                 let r = [
//                     SymbolAndPos {
//                         symbol: (&s.as_str()[0..index]).to_string(),
//                         pos: pos.clone(),
//                     },
//                     SymbolAndPos {
//                         symbol: (&s.as_str()[index + 1..]).to_string(),
//                         pos: Pos {
//                             file: pos.file,
//                             offset: pos.offset + index + 1,
//                             line: pos.line,
//                             col: pos.col + index + 1,
//                         },
//                     },
//                 ];
//                 Self::Two(r)
//             }
//             None => Self::One(SymbolAndPos {
//                 symbol: s.clone(),
//                 pos: pos.clone(),
//             }),
//         }
//     }
// }

// #[cfg(test)]
// #[test]
// fn test_splited_symbol() {
//     {
//         let s = "xxx.yyy";
//         let pos = Pos {
//             file: 2,
//             offset: 0,
//             // line: 0,
//             // col: 0,
//         };
//         let x = SplitedSymbol::from(&(s.to_string(), pos));
//         assert_eq!(
//             x,
//             SplitedSymbol::Two([
//                 SymbolAndPos {
//                     symbol: "xxx".to_string(),
//                     pos
//                 },
//                 SymbolAndPos {
//                     symbol: "yyy".to_string(),
//                     pos: Pos {
//                         file: pos.file,
//                         offset: pos.offset + 4,
//                         line: pos.line,
//                         col: pos.col + 4,
//                     }
//                 }
//             ])
//         );
//     }
//     {
//         let s = "xxx";
//         let pos = Pos {
//             file: 2,
//             offset: 0,
//             line: 0,
//             col: 0,
//         };
//         let x = SplitedSymbol::from(&(s.to_string(), pos));
//         assert_eq!(
//             x,
//             SplitedSymbol::One(SymbolAndPos {
//                 symbol: s.to_string(),
//                 pos
//             })
//         );
//     }
// }
