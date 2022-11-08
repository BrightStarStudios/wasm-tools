use anyhow::{Result, bail};
use regex::Regex;
use std::{io::Write, fmt};
use std::ops::Range;
use std::collections::HashMap;
use wasmparser::{Encoding, Parser, Payload::*, SectionReader, ValType, SectionWithLimitedItems, Operator};

/// Dumps information about sections in a WebAssembly file.
///
/// This is a relatively incomplete subcommand and is generally intended to just
/// help poke around an object file.
#[derive(clap::Parser)]
pub struct Opts {
    #[clap(flatten)]
    io: wasm_tools::InputOutput,
}

#[derive(Clone, Copy)]
pub struct FunctionInfo {
    id: u32,
    call_count: u32,
    byte_size: usize,
}

impl Opts {
    pub fn run(&self) -> Result<()> {
        let input = self.io.parse_input_wasm()?;

        let mut printer = Printer {
            indices: Vec::new(),
            output: self.io.output_writer()?,
        };
        printer.indices.push(IndexSpace::default());

        let mut functions_info = Vec::new();
        // let mut functionInfoMap = HashMap::new();
        let mut functions_name_map = HashMap::new();
        let mut types_name_map = HashMap::new();
        let mut code_section_counter = 0;

        for payload in Parser::new(0).parse_all(&input) {
            match payload? {
                Version { .. } => {}

                TypeSection(s) => {

                    
                    printer.section(s, "types")?

                },
                ImportSection(s) => printer.section(s, "imports")?,
                FunctionSection(mut s) => {

                    let mut already_owned_count = 0;
                    println!("Function section. Has: {} count", s.get_count());
                    for _ in 0..s.get_count() {
                        let id = s.read()?;
                        functions_info.push(FunctionInfo{ id: id, call_count: 0, byte_size: 0 });
                    }
                    if !s.eof() {
                        bail!("too many bytes in section");
                    }

                    println!("Unique func signatures: {} - Duplicate function signatures {}", functions_info.len(), already_owned_count);

                    printer.section(s, "functions")?
                },
                TableSection(s) => printer.section(s, "tables")?,
                MemorySection(s) => printer.section(s, "memories")?,
                TagSection(s) => printer.section(s, "tags")?,
                GlobalSection(mut s) => {
                    
                    for _ in 0..s.get_count() {
                        let result = s.read()?;
                        println!("Global type: {:?} (mutable: {})", result.ty.content_type, result.ty.mutable);
                    }

                    printer.section(s, "globals")?;
                },
                ExportSection(s) => printer.section(s, "exports")?,
                StartSection { range, .. } => printer.section_raw(range, 1, "start")?,
                ElementSection(mut s) => {
                    
                    for _ in 0..s.get_count() {
                        let res = s.read()?;
                        match res.kind {
                            wasmparser::ElementKind::Passive => {
                                // Ignored
                            },
                            wasmparser::ElementKind::Active { table_index, offset_expr } => {
                                // let reader = offset_expr.get_binary_reader();
                                // reader.
                                println!("Active type. Table index {}", table_index);
                            },
                            wasmparser::ElementKind::Declared => {
                                if res.ty == ValType::FuncRef {
                                    // println!("Func declared. Items count {}");
                                }
                            },
                        }
                    }


                    printer.section(s, "elements")?
                },
                DataCountSection { range, .. } => printer.section_raw(range, 1, "data count")?,
                DataSection(s) => printer.section(s, "data")?,
                CodeSectionStart { range, count, .. } => {
                    printer.section_raw(range, count, "code")?
                }
                CodeSectionEntry(mut s) => {
                    functions_info[code_section_counter].byte_size = s.range().end - s.range().start;

                    let mut reader = s.get_operators_reader()?;

                    while true {
                        let read = reader.read_with_offset();
                        match read {
                            Ok(val) => {
                                match val.0 {
                                    Operator::Call { function_index } => {
                                        if function_index as usize >= functions_info.len() {
                                            println!("Invalid function call??: {} (skipped)", function_index);
                                            continue;
                                        }

                                        functions_info[function_index as usize].call_count += 1;
                                    },
                                    _ => {}
                                }
                            },
                            Err(_) => {
                                // println!("Reached end of reader...? err: {}", err.message());
                                break;
                            },
                        }
                    }

                    code_section_counter += 1;
                }

                ModuleSection { range, .. } => {
                    printer.section_raw(range, 1, "module")?;
                    printer.start(Encoding::Module)?;
                }
                InstanceSection(s) => printer.section(s, "core instances")?,
                CoreTypeSection(s) => printer.section(s, "core types")?,
                ComponentSection { range, .. } => {
                    printer.section_raw(range, 1, "component")?;
                    printer.indices.push(IndexSpace::default());
                    printer.start(Encoding::Component)?;
                }
                ComponentInstanceSection(s) => printer.section(s, "component instances")?,
                ComponentAliasSection(s) => printer.section(s, "component alias")?,
                ComponentTypeSection(s) => printer.section(s, "component types")?,
                ComponentCanonicalSection(s) => printer.section(s, "canonical functions")?,
                ComponentStartSection(s) => printer.section_raw(s.range(), 1, "component start")?,
                ComponentImportSection(s) => printer.section(s, "component imports")?,
                ComponentExportSection(s) => printer.section(s, "component exports")?,
                
                CustomSection(c) => {
                    if c.name() == "name" {
                        println!("custom section name: {}", c.name());
                        
                        // let mut functionNameCount = 0;
                        // let mut localNameCount = 0;
                        // let mut labelNameCount = 0;
                        // let mut typeNameCount = 0;
                        // let mut tableNameCount = 0;
                        // let mut memoryNameCount = 0;
                        // let mut globalNameCount = 0;
                        // let mut elementNameCount = 0;
                        // let mut dataNameCount = 0;

                        let mut nameiter = wasmparser::NameSectionReader::new(c.data(), c.data_offset())?;
                        while !nameiter.eof() {
                            let name = nameiter.read()?;
                            let range = nameiter.original_position();

                            match name {
                                wasmparser::Name::Module { name, name_range } => {
                                    println!("module name");
                                    println!("{}", name_range.start);
                                    println!("{:?}", name);
                                    println!("{}", name_range.end);
                                }
                                wasmparser::Name::Function(mut n) => {
                                    for _ in 0..n.get_count() {
                                        let item = n.read()?;
                                        if functions_name_map.contains_key(&item.index) {
                                            bail!("function name already in lookup");
                                        }

                                        functions_name_map.insert(item.index, item.name);
                                    }
                                    if !n.eof() {
                                        bail!("too many bytes in section");
                                    }
                                },
                                wasmparser::Name::Local(n) => {
                                    // self.print_indirect_name_map("function", "local", n)?
                                },
                                wasmparser::Name::Label(n) => {
                                    // self.print_indirect_name_map("function", "label", n)?
                                },
                                wasmparser::Name::Type(mut n) => {
                                    for _ in 0..n.get_count() {
                                        let item = n.read()?;
                                        if types_name_map.contains_key(&item.index) {
                                            bail!("type name already in lookup");
                                        }

                                        types_name_map.insert(item.index, item.name);
                                    }
                                    if !n.eof() {
                                        bail!("too many bytes in section");
                                    }

                                    // self.print_name_map("type", n)?
                                },
                                wasmparser::Name::Table(n) => {
                                    // self.print_name_map("table", n)?
                                },
                                wasmparser::Name::Memory(n) => {
                                    // self.print_name_map("memory", n)?
                                },
                                wasmparser::Name::Global(n) => {
                                    // self.print_name_map("global", n)?
                                },
                                wasmparser::Name::Element(n) => {
                                    // self.print_name_map("element", n)?
                                },
                                wasmparser::Name::Data(n) => {
                                    // self.print_name_map("data", n)?
                                },
                                wasmparser::Name::Unknown { ty, range, .. } => {
                                    // write!(self.state, "unknown names: {}", ty)?;
                                    // self.print(range.start)?;
                                    // self.print(end)?;
                                }
                            }
                        }
                    }

                    printer.section_raw(
                        c.data_offset()..c.data_offset() + c.data().len(),
                        1,
                        &format!("custom {:?}", c.name()),
                    )?
                },

                UnknownSection { .. } => {}

                End(_) => printer.end()?,
            }
        }


        if functions_name_map.len() == 0 {
            bail!("Can only parse wasm files with custom 'name' section. Make sure the wasm has debug names in the file.");
        }

        let mut funcs_by_namespace = HashMap::new();
        funcs_by_namespace.insert("non_namespaced", Vec::new());

        // let recognized_namespace = ["entt", "es", "std", "physx", "draco", "spdlog"];
        
        let namespace_reg = Regex::new(r"(?:\s)((?:[a-z0-9-_]+)(?:::)?(?:[a-z0-9-_]+))").expect("regex failed to compile");
        for index in 0..function_info.len() {
            let index_u32 = index as u32;
            let info = function_info[index];
            let name = function_names_map.get(&index_u32).expect("Couldn't find key in name");
            
            let captures_a = namespace_reg.captures(name);
            match captures_a {
                Some(captures) => {
                    
                    let mut namespace = captures.get(1).unwrap().as_str();
                    namespace = namespace.trim();
                    
                    if namespace.starts_with("es::") == false {
                        let mut splitNamespace = namespace.split("::");
                        namespace = splitNamespace.next().unwrap(); // If it's not es:: then just go 1 layer deep
                    }

                    if funcs_by_namespace.contains_key(namespace) {
                        let v : &mut Vec<FunctionInfo> = funcs_by_namespace.get_mut(namespace).unwrap();
                        v.push(info);
                    } else {
                        let mut v = Vec::new();
                        v.push(info);
                        funcs_by_namespace.insert(namespace, v);
                    }
                }
                None => {
                    let v : &mut Vec<FunctionInfo> = funcs_by_namespace.get_mut("non_namespaced").unwrap();
                    v.push(info);
                }
            }
        }

        let mut called_funcs_count = 0;
        let mut uncalled_funcs_count = 0;
        for index in 0..functions_info.len() {
            let info = &functions_info[index];

            if info.call_count > 0 {
                called_funcs_count += 1;
            } else {
                uncalled_funcs_count += 1;
            }
        }

        println!("Called funcs: {}, uncalled funcs: {}", called_funcs_count, uncalled_funcs_count);


        let mut namespaces_ordered : Vec<(&&str, &Vec<FunctionInfo>)> = funcs_by_namespace.iter().collect();
        namespaces_ordered.sort_by(|a, b| {
            let mut byte_size_a = 0usize;
            for k in a.1 {
                byte_size_a += k.byte_size;
            }

            let mut byteSizeB = 0usize;
            for k in b.1 {
                byteSizeB += k.byte_size;
            }
            
            if byte_size_a == byteSizeB {
                return std::cmp::Ordering::Equal;
            }

            if byte_size_a > byteSizeB {
                return std::cmp::Ordering::Greater;
            }

            return std::cmp::Ordering::Less;
        });

        println!("{} namespaces found", namespaces_ordered.len());
        for tuple in namespaces_ordered {
            let mut byteSize = 0usize;
            for k in tuple.1 {
                byteSize += k.byte_size;
            }

            println!("'{}' functions: {} ({} bytes)", tuple.0, tuple.1.len(), byteSize);
        }

        Ok(())
    }
}

#[derive(Default)]
struct IndexSpace {
    modules: u32,
    components: u32,
    processing: Vec<Encoding>,
}

struct Printer {
    indices: Vec<IndexSpace>,
    output: Box<dyn Write>,
}

impl Printer {
    fn start(&mut self, encoding: Encoding) -> Result<()> {
        if let Some(space) = self.indices.last_mut() {
            space.processing.push(encoding);
        }

        if let Some(space) = self.indices.last() {
            match encoding {
                Encoding::Module => {
                    writeln!(
                        self.output,
                        "{}------ start module {} -------------",
                        self.header(),
                        space.modules
                    )?;
                }
                Encoding::Component => {
                    writeln!(
                        self.output,
                        "{}------ start component {} ----------",
                        self.header(),
                        space.components
                    )?;
                }
            }
        }
        Ok(())
    }

    fn end(&mut self) -> Result<()> {
        let header = self.header();
        if let Some(space) = self.indices.last_mut() {
            match space.processing.pop() {
                Some(Encoding::Module) => {
                    writeln!(
                        self.output,
                        "{}------ end module {} -------------",
                        header, space.modules
                    )?;
                    space.modules += 1;
                }
                Some(Encoding::Component) => {
                    writeln!(
                        self.output,
                        "{}------ end component {} ----------",
                        header, space.components
                    )?;
                    self.indices.pop();

                    if let Some(space) = self.indices.last_mut() {
                        space.components += 1;
                    }
                }
                None => {
                    self.indices.pop();
                }
            }
        }
        Ok(())
    }

    fn section<T>(&mut self, section: T, name: &str) -> Result<()>
    where
        T: wasmparser::SectionWithLimitedItems + wasmparser::SectionReader,
    {
        self.section_raw(section.range(), section.get_count(), name)
    }

    fn section_raw(&mut self, range: Range<usize>, count: u32, name: &str) -> Result<()> {
        writeln!(
            self.output,
            "{:40} | {:#10x} - {:#10x} | {:9} bytes | {} count",
            format!("{}{}", self.header(), name),
            range.start,
            range.end,
            range.end - range.start,
            count,
        )?;
        Ok(())
    }

    fn header(&self) -> String {
        let mut s = String::new();
        let depth = self
            .indices
            .last()
            .map_or(0, |space| match space.processing.last() {
                Some(Encoding::Module) => self.indices.len() + 1,
                _ => self.indices.len(),
            });

        for _ in 0..depth {
            s.push_str("  ");
        }
        return s;
    }
}
