use std::collections::HashMap;
use std::error::Error as StdError;
use std::fs::{File, read};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::{cmp, process};
use structopt::StructOpt;
use sv_parser::{parse_sv, SyntaxTree, unwrap_node, Locate, RefNode, Define, DefineText};
use sv_parser_error;
use sv_parser_syntaxtree::*;
use enquote;
use tempfile::NamedTempFile;

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

    /// Show the full syntax tree rather than just module instantiation
    #[structopt(long = "full-tree")]
    pub full_tree: bool,

    /// Include whitespace in output syntax tree
    #[structopt(long = "include-whitespace")]
    pub include_whitespace: bool,
 
    /// Show the macro definitions after processing each file
    #[structopt(long = "show-macro-defs")]
    pub show_macro_defs: bool,

    /// Treat each file as completely separate, not updating define variables after each file
    #[structopt(long = "separate")]
    pub separate: bool,

    /// Allow incomplete
    #[structopt(long = "allow_incomplete")]
    pub allow_incomplete: bool
}

fn main() {
    let opt = Opt::from_args();
    let exit_code = run_opt(&opt);
    process::exit(exit_code);
}

fn run_opt(
    opt: &Opt
) -> i32 {

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
    
    // flag to determine parsing status
    let mut exit_code = 0;
    
    // parse files
    println!("files:");
    for path in &opt.files {
        // use temporary files to sanitize non-ASCII characters
        let Ok(mut tmpfile) = NamedTempFile::new() else { continue; };
        let Ok(org) = read(&path) else { continue; };
        let org_string : String = org.iter().map(|&c| if c < 128 { c as char } else { '?' }).collect();
        let _ = tmpfile.write_all(org_string.as_bytes());

        match parse_sv(tmpfile.path(), &defines, &opt.includes, opt.ignore_include, opt.allow_incomplete) {
            Ok((syntax_tree, new_defines)) => {
                let _ = tmpfile.close();
                println!("  - file_name: {}", escape_str(path.to_str().unwrap()));
                if !opt.full_tree {
                    println!("    defs:");
                    analyze_defs(&syntax_tree);
                } else {
                    println!("    syntax_tree:");
                    print_full_tree(&syntax_tree, opt.include_whitespace);
                }
                // update the preprocessor state if desired
                if !opt.separate {
                    defines = new_defines;
                }
                // show macro definitions if desired
                if opt.show_macro_defs {
                    println!("    macro_defs:");
                    show_macro_defs(&defines);
                }
            }
            Err(x) => {
                match x {
                    sv_parser_error::Error::Parse(Some((origin_path, origin_pos))) => {
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
                exit_code = 1;
            }
        }
    }
    
    // return exit code
    exit_code
}

static CHAR_CR: u8 = 0x0d;
static CHAR_LF: u8 = 0x0a;

fn print_parse_error(
    origin_path: &PathBuf,
    origin_pos: &usize
) {
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

fn show_macro_defs(
    defines: &HashMap<String, Option<Define>>
) {
    for (_, value) in defines.into_iter() {
        match value {
            Some(define) => println!("      - '{:?}'", define),
            _ => (),
        }
    }
}

// ==== rewritten definition analyzer starts from here ====
struct DefsState {
    first_port: bool,
    first_inst: bool,
    is_input: bool,
    port_width: i32
}

// module definition
fn process_module_def(
    syntax_tree: &SyntaxTree,
    node: RefNode,
    s: &mut DefsState
) {
    let Some(id) = unwrap_node!(node, ModuleIdentifier) else { return; };
    let Some(id) = get_identifier(id) else { return; };      
    // Original string can be got by SyntaxTree::get_str(self, node: &RefNode)
    let Some(id) = syntax_tree.get_str(&id) else { return; }; 
    // Declare the new module
    if s.first_port {
        println!("        ports: []");
    }
    if s.first_inst {
        println!("        insts: []");
    }
    println!("      - mod_name: {}", escape_str(id));
    s.first_port = true;
    s.first_inst = true;
}

// module instantiation
fn process_module_inst(
    syntax_tree: &SyntaxTree,
    node: RefNode,
    s: &mut DefsState
) {
    // write the module name
    let Some(id) = unwrap_node!(node.clone(), ModuleIdentifier) else { return; };
    let Some(id) = get_identifier(id) else { return; };      
    let Some(id) = syntax_tree.get_str(&id) else { return; }; 
    if s.first_inst {
        println!("        insts:");
        s.first_inst = false;
    }
    println!("          - mod_name: {}", escape_str(id));
    // write the instance name
    let Some(id) = unwrap_node!(node, InstanceIdentifier) else { return; };
    let Some(id) = get_identifier(id) else { return; };      
    let Some(id) = syntax_tree.get_str(&id) else { return; }; 
    println!("            inst_name: {}", escape_str(id));
}

// port definition (direction and width)
fn process_port_def(
    syntax_tree: &SyntaxTree,
    node: RefNode,
    s: &mut DefsState
) {
    'check_direction1: {
        let Some(id) = unwrap_node!(node.clone(), PortDirection) else { break 'check_direction1; };
        let Some(id) = get_keyword(id) else { break 'check_direction1; };      
        let Some(id) = syntax_tree.get_str(&id) else { break 'check_direction1; }; 
        s.is_input = id == "input";
        s.port_width = 1;
    }
    'check_direction2: {
        let Some(_) = unwrap_node!(node.clone(), InputDeclaration) else { break 'check_direction2; };
        s.is_input = true;
        s.port_width = 1;
    }
    'check_direction3: {
        let Some(_) = unwrap_node!(node.clone(), OutputDeclaration) else { break 'check_direction3; };
        s.is_input = false;
        s.port_width = 1;
    }
    'check_range: {
        let Some(id) = unwrap_node!(node.clone(), ConstantRange) else { break 'check_range; };
        let Some(id) = get_unsigned_number(id) else { break 'check_range; };      
        let Some(id) = syntax_tree.get_str(&id) else { break 'check_range; };
        s.port_width = id.parse::<i32>().unwrap() + 1;
    }
    for x in node {
        match x {
            RefNode::PortIdentifier(x) => process_port_ident(syntax_tree, RefNode::from(x), s),
            _ => ()
        }
    }
}

// port identifier
fn process_port_ident(
    syntax_tree: &SyntaxTree,
    node: RefNode,
    s: &mut DefsState
) {
    let Some(id) = get_identifier(node) else { return; };
    let Some(id) = syntax_tree.get_str(&id) else { return; };
    if s.first_port {
        println!("        ports:");
        s.first_port = false;
    }
    println!("          - port_name: {}", escape_str(id));
    if s.is_input {
        println!("            port_dir: \"input\"");
    } else {
        println!("            port_dir: \"output\"");
    }
    println!("            port_width: {}", s.port_width);
}

fn analyze_defs(
    syntax_tree: &SyntaxTree
) {
    let mut s = DefsState {
        first_port: false,
        first_inst: false,
        is_input: true,
        port_width: 1
    };
    // &SyntaxTree is iterable
    for node in syntax_tree {
        // The type of each node is RefNode
        match node {
            RefNode::ModuleDeclarationNonansi(x) => {
                // unwrap_node! gets the nearest ModuleIdentifier from x
                process_module_def(syntax_tree, RefNode::from(x), &mut s);
            }
            RefNode::ModuleDeclarationAnsi(x) => {
                process_module_def(syntax_tree, RefNode::from(x), &mut s);
            }
            RefNode::ModuleInstantiation(x) => {
                process_module_inst(syntax_tree, RefNode::from(x), &mut s);
            }
            RefNode::AnsiPortDeclaration(x) => {
                process_port_def(syntax_tree, RefNode::from(x), &mut s);
            }
            RefNode::PortDeclaration(x) => {
                process_port_def(syntax_tree, RefNode::from(x), &mut s);
            }
            _ => (),
        }
    }
    if s.first_port {
        println!("        ports: []");
    }
    if s.first_inst {
        println!("        insts: []");
    }
}
// ==== rewritten definition analyzer ends here ====

fn print_full_tree(
    syntax_tree: &SyntaxTree,
    include_whitespace: bool
) {
    let mut skip = false;
    let mut depth = 3;
    for node in syntax_tree.into_iter().event() {
        match node {
            NodeEvent::Enter(RefNode::Locate(locate)) => {
                if !skip {
                    println!("{}- Token: {}",
                             "  ".repeat(depth),
                             escape_str(syntax_tree.get_str(locate).unwrap()));
                    println!("{}  Line: {}",
                             "  ".repeat(depth),
                             locate.line);
                }
                depth += 1;
            }
            NodeEvent::Enter(RefNode::WhiteSpace(_)) => {
                if !include_whitespace {
                    skip = true;
                }
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

fn get_identifier(
    node: RefNode
) -> Option<Locate> {
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

fn get_keyword(
    node: RefNode
) -> Option<Locate> {
    match unwrap_node!(node, Keyword) {
        Some(RefNode::Keyword(x)) => {
            return Some(x.nodes.0);
        }
        _ => None,
    }
}

fn get_unsigned_number(
    node: RefNode
) -> Option<Locate> {
    match unwrap_node!(node, UnsignedNumber) {
        Some(RefNode::UnsignedNumber(x)) => {
            return Some(x.nodes.0);
        }
        _ => None,
    }
}

// escape_str adapted from this code:
// https://github.com/chyh1990/yaml-rust/blob/6cd3ce4abe6894443645c48bdc375808ec911493/src/emitter.rs#L43-L104
fn escape_str(v: &str) -> String {
    let mut wr = String::new();
    
    wr.push_str("\"");

    let mut start = 0;

    for (i, byte) in v.bytes().enumerate() {
        let escaped = match byte {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\x00' => "\\u0000",
            b'\x01' => "\\u0001",
            b'\x02' => "\\u0002",
            b'\x03' => "\\u0003",
            b'\x04' => "\\u0004",
            b'\x05' => "\\u0005",
            b'\x06' => "\\u0006",
            b'\x07' => "\\u0007",
            b'\x08' => "\\b",
            b'\t' => "\\t",
            b'\n' => "\\n",
            b'\x0b' => "\\u000b",
            b'\x0c' => "\\f",
            b'\r' => "\\r",
            b'\x0e' => "\\u000e",
            b'\x0f' => "\\u000f",
            b'\x10' => "\\u0010",
            b'\x11' => "\\u0011",
            b'\x12' => "\\u0012",
            b'\x13' => "\\u0013",
            b'\x14' => "\\u0014",
            b'\x15' => "\\u0015",
            b'\x16' => "\\u0016",
            b'\x17' => "\\u0017",
            b'\x18' => "\\u0018",
            b'\x19' => "\\u0019",
            b'\x1a' => "\\u001a",
            b'\x1b' => "\\u001b",
            b'\x1c' => "\\u001c",
            b'\x1d' => "\\u001d",
            b'\x1e' => "\\u001e",
            b'\x1f' => "\\u001f",
            b'\x7f' => "\\u007f",
            _ => continue,
        };

        if start < i {
            wr.push_str(&v[start..i]);
        }

        wr.push_str(escaped);

        start = i + 1;
    }

    if start != v.len() {
        wr.push_str(&v[start..]);
    }

    wr.push_str("\"");
    
    wr
}
