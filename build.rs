mod oxc_semantic {
    pub struct SemanticBuilder;

    impl SemanticBuilder {
        pub fn new() -> Self {
            SemanticBuilder
        }
    }
}

use oxc_semantic::SemanticBuilder;

fn main() {
    let _builder = SemanticBuilder::new();
}
