use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

fn top_level_bindings(source: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let exported: std::collections::HashSet<&str> = ret
        .module_record
        .local_export_entries
        .iter()
        .filter_map(|e| e.local_name.name())
        .map(|s| s.as_str())
        .collect();

    let semantic = SemanticBuilder::new().build(&ret.program).semantic;
    let scoping = semantic.scoping();
    let root = scoping.root_scope_id();
    scoping
        .get_bindings(root)
        .iter()
        .filter_map(|(ident, &symbol_id)| {
            let name = ident.as_str();
            let flags = scoping.symbol_flags(symbol_id);
            if flags.is_import() || exported.contains(name) {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}
fn main() {}
