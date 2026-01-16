# Completion Lists Refactoring Design

**Date:** 2026-01-16
**Goal:** Move hardcoded completion lists (units, ingredients, cookware) from Rust constants to embedded text files

## Motivation

- **Easier maintenance:** Add/update items without touching Rust code
- **Better organization:** Separate data from logic
- **Cleaner architecture:** Data files can be documented and version-controlled independently

## File Structure

```
cooklang-language-server/
├── data/
│   ├── units.txt           # Cooking measurement units
│   ├── time_units.txt      # Time units for timers
│   ├── ingredients.txt     # Common ingredients
│   └── cookware.txt        # Common cookware items
├── src/
│   └── completion.rs
└── Cargo.toml
```

## File Formats

### Units Files (units.txt, time_units.txt)

Key-value pairs with `=` separator:
```
g = grams
kg = kilograms
ml = milliliters
# Comments allowed for organization
cup = cups
```

### Simple Lists (ingredients.txt, cookware.txt)

One item per line:
```
salt
pepper
olive oil
# Comments for sectioning
butter
garlic
```

## Implementation Approach

### 1. Embedding Strategy

Use `include_str!` macro to embed files at compile time:

```rust
const UNITS_DATA: &str = include_str!("../data/units.txt");
const TIME_UNITS_DATA: &str = include_str!("../data/time_units.txt");
const INGREDIENTS_DATA: &str = include_str!("../data/ingredients.txt");
const COOKWARE_DATA: &str = include_str!("../data/cookware.txt");
```

**Benefits:**
- Zero runtime overhead
- Binary includes all data (no external file dependencies)
- Simple and fast

### 2. Parsing Functions

```rust
fn parse_unit_pairs(data: &str) -> Vec<(&str, &str)> {
    data.lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('=').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        .collect()
}

fn parse_simple_list(data: &str) -> Vec<&str> {
    data.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}
```

### 3. Lazy Initialization

Use `std::sync::LazyLock` (Rust 1.80+) or `once_cell` for older versions:

```rust
use std::sync::LazyLock;

static UNITS: LazyLock<Vec<(&str, &str)>> = LazyLock::new(|| {
    parse_unit_pairs(include_str!("../data/units.txt"))
});

static TIME_UNITS: LazyLock<Vec<(&str, &str)>> = LazyLock::new(|| {
    parse_unit_pairs(include_str!("../data/time_units.txt"))
});

static COMMON_INGREDIENTS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    parse_simple_list(include_str!("../data/ingredients.txt"))
});

static COMMON_COOKWARE: LazyLock<Vec<&str>> = LazyLock::new(|| {
    parse_simple_list(include_str!("../data/cookware.txt"))
});
```

### 4. Error Handling

Malformed lines are silently skipped during parsing. This makes the system resilient to formatting errors while maintaining simplicity.

## Migration Steps

1. **Create data directory and files**
   - `mkdir data/`
   - Extract existing constants into respective text files
   - Add organizing comments

2. **Check Rust version**
   - If Rust 1.80+: use `std::sync::LazyLock`
   - If older: add `once_cell = "1.19"` to Cargo.toml

3. **Refactor completion.rs**
   - Remove hardcoded const arrays
   - Add `include_str!` macros
   - Add parsing functions
   - Convert to lazy statics
   - No changes needed to completion functions (they work with slices)

4. **Test**
   - Verify completions still work
   - Check that all items are present
   - Validate parsing handles comments correctly

## Backward Compatibility

No breaking changes:
- Completion API remains identical
- Data structure types unchanged (`&[(&str, &str)]` for units, `&[&str]` for lists)
- Completion functions work as-is

## Future Enhancements

Potential future improvements (not in scope for this refactor):
- Runtime override support (load custom files from config directory)
- More metadata in data files (descriptions, categories)
- Localization support
- User-defined custom lists
