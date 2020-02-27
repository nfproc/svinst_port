use std::collections::HashMap;
use std::error::Error as StdError;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{cmp, process};
use structopt::StructOpt;
use sv_parser::{parse_sv, SyntaxTree, unwrap_node, Locate, RefNode, Define, DefineText};
use sv_parser_error::Error;
use sv_parser_syntaxtree::*;
use enquote;

#[derive(StructOpt)]
struct Opt {
    pub files: Vec<PathBuf>,

    /// Define
    #[structopt(short = "d", long = "define", multiple = true, number_of_values = 1)]
    pub defines: Vec<String>,

    /// Include path
    #[structopt(short = "i", long = "include", multiple = true, number_of_values = 1)]
    pub includes: Vec<PathBuf>,

    /// Ignore any include
    #[structopt(long = "ignore-include")]
    pub ignore_include: bool,

    /// Ignore any include
    #[structopt(long = "full-tree")]
    pub full_tree: bool
}


fn main() {
    let opt = Opt::from_args();

    // read in define variables
    let mut defines = HashMap::new();
    for define in &opt.defines {
		let mut define = define.splitn(2, '=');
        let ident = String::from(define.next().unwrap());
        let text = if let Some(x) = define.next() {
            let x = enquote::unescape(x, None).unwrap();
            Some(DefineText::new(x, None))
        } else {
            None
        };
        let define = Define::new(ident.clone(), vec![], text);
        defines.insert(ident, Some(define));
	}
    
    // parse files
    println!("files:");
    for path in &opt.files {
        match parse_sv(&path, &defines, &opt.includes, opt.ignore_include) {
            Ok((syntax_tree, _new_defines)) => {
				println!("  - file_name: \"{}\"", path.to_str().unwrap());
				if !opt.full_tree {
					println!("    mod_defs:");
					analyze_mod_defs(&syntax_tree);
				} else {
					println!("    syntax_tree:");
					print_full_tree(&syntax_tree);
				}
            }
            Err(x) => {
                match x {
                    Error::Parse(Some((origin_path, origin_pos))) => {
                        eprintln!("parse failed: {:?}", path);
                        print_parse_error(&origin_path, &origin_pos);
                    }
                    x => {
                        eprintln!("parse failed: {:?} ({})", path, x);
                        let mut err = x.source();
                        while let Some(x) = err {
                            eprintln!("  Caused by {}", x);
                            err = x.source();
                        }
                    }
                }
				process::exit(1);
            }
        }
    }

    // exit when done
    process::exit(0);
}

static CHAR_CR: u8 = 0x0d;
static CHAR_LF: u8 = 0x0a;

fn print_parse_error(origin_path: &PathBuf, origin_pos: &usize) {
    let mut f = File::open(&origin_path).unwrap();
    let mut s = String::new();
    let _ = f.read_to_string(&mut s);

    let mut pos = 0;
    let mut column = 1;
    let mut last_lf = None;
    while pos < s.len() {
        if s.as_bytes()[pos] == CHAR_LF {
            column += 1;
            last_lf = Some(pos);
        }
        pos += 1;

        if *origin_pos == pos {
            let row = if let Some(last_lf) = last_lf {
                pos - last_lf
            } else {
                pos + 1
            };
            let mut next_crlf = pos;
            while next_crlf < s.len() {
                if s.as_bytes()[next_crlf] == CHAR_CR || s.as_bytes()[next_crlf] == CHAR_LF {
                    break;
                }
                next_crlf += 1;
            }

            let column_len = format!("{}", column).len();

            eprint!(" {}:{}:{}\n", origin_path.to_string_lossy(), column, row);

            eprint!("{}|\n", " ".repeat(column_len + 1));

            eprint!("{} |", column);

            let beg = if let Some(last_lf) = last_lf {
                last_lf + 1
            } else {
                0
            };
            eprint!(
                " {}\n",
                String::from_utf8_lossy(&s.as_bytes()[beg..next_crlf])
            );

            eprint!("{}|", " ".repeat(column_len + 1));

            eprint!(
                " {}{}\n",
                " ".repeat(pos - beg),
                "^".repeat(cmp::min(origin_pos + 1, next_crlf) - origin_pos)
            );
        }
    }
}

fn analyze_mod_defs(syntax_tree: &SyntaxTree) {
    // &SyntaxTree is iterable
    for node in syntax_tree {
        // The type of each node is RefNode
        match node {
            RefNode::ModuleDeclarationNonansi(x) => {
                // unwrap_node! gets the nearest ModuleIdentifier from x
                let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                let id = get_identifier(id).unwrap();
                // Original string can be got by SyntaxTree::get_str(self, node: &RefNode)
                let id = syntax_tree.get_str(&id).unwrap();
                // Declare the new module
				println!("      - mod_name: \"{}\"", id);
				println!("        mod_insts:");
            }
            RefNode::ModuleDeclarationAnsi(x) => {
                let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                let id = get_identifier(id).unwrap();
                let id = syntax_tree.get_str(&id).unwrap();
				println!("      - mod_name: \"{}\"", id);
				println!("        mod_insts:");
            }
            RefNode::ModuleInstantiation(x) => {
				// write the module name
				let id = unwrap_node!(x, ModuleIdentifier).unwrap();
				let id = get_identifier(id).unwrap();
                let id = syntax_tree.get_str(&id).unwrap();
                println!("          - mod_name: \"{}\"", id);
                // write the instance name
				let id = unwrap_node!(x, InstanceIdentifier).unwrap();
				let id = get_identifier(id).unwrap();
                let id = syntax_tree.get_str(&id).unwrap();
                println!("            inst_name: \"{}\"", id);
			}
            _ => (),
        }
    }
}

fn print_full_tree(syntax_tree: &SyntaxTree) {
	let mut skip = false;
	let mut depth = 3;
	for node in syntax_tree.into_iter().event() {
		match node {
			NodeEvent::Enter(RefNode::Locate(locate)) => {
				if !skip {
					println!("{}- Token: \"{}\"",
					         "  ".repeat(depth),
					         syntax_tree.get_str(locate).unwrap());
					println!("{}  Line: {}",
					         "  ".repeat(depth),
					         locate.line);
				}
				depth += 1;
			}
			NodeEvent::Enter(RefNode::WhiteSpace(_)) => {
				skip = true;
			}
			NodeEvent::Leave(RefNode::WhiteSpace(_)) => {
				skip = false;
			}
			NodeEvent::Enter(x) => {
				if !skip {
					println!("{}- {}:",
					         "  ".repeat(depth),
					         x);
				}
				depth += 1;
			}
			NodeEvent::Leave(_) => {
				depth -= 1;
			}
		}
	}
}

fn get_identifier(node: RefNode) -> Option<Locate> {
    // unwrap_node! can take multiple types
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        Some(RefNode::EscapedIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        _ => None,
    }
}

