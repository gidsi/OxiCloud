import sys

with open("Cargo.toml", "r") as f:
    lines = f.readlines()

new_lines = []
in_dependencies = False
in_build_dependencies = False

for line in lines:
    new_lines.append(line)
    if line.strip() == "[dependencies]":
        in_dependencies = True
        in_build_dependencies = False
        new_lines.append('smol_str = "0.2"\n')
        new_lines.append('async-trait = "0.1"\n')
        new_lines.append('askama = { version = "0.12", features = ["with-axum"] }\n')
        new_lines.append('lettre = { version = "0.11", features = ["tokio1", "smtp-transport", "builder"] }\n')
    elif line.strip() == "[build-dependencies]":
        in_build_dependencies = True
        in_dependencies = False
        new_lines.append('oxc_semantic = "0.125.0"\n')

with open("Cargo.toml", "w") as f:
    f.writelines(new_lines)
