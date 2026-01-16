# Completion Lists Refactoring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor hardcoded completion lists in completion.rs to use embedded text files

**Architecture:** Extract UNITS, TIME_UNITS, COMMON_INGREDIENTS, and COMMON_COOKWARE constants into separate text files in a data/ directory. Use include_str! to embed at compile time and std::sync::LazyLock for lazy parsing.

**Tech Stack:** Rust 1.92.0, std::sync::LazyLock

---

## Task 1: Create data directory and units.txt

**Files:**
- Create: `data/units.txt`

**Step 1: Create data directory**

```bash
mkdir -p data
```

**Step 2: Create units.txt with weight and volume units**

Create `data/units.txt`:

```txt
# Weight units
g = grams
kg = kilograms
mg = milligrams

# Volume units
ml = milliliters
l = liters

# Imperial units
oz = ounces
lb = pounds

# Cooking measurements
cup = cups
cups = cups
tbsp = tablespoons
tsp = teaspoons

# Count/portion units
pinch = pinch
clove = cloves
cloves = cloves
slice = slices
slices = slices
piece = pieces
pieces = pieces
bunch = bunches
sprig = sprigs

# Container units
can = cans
jar = jars
packet = packets
head = heads
stalk = stalks
```

**Step 3: Verify file created**

```bash
ls -la data/units.txt
cat data/units.txt
```

Expected: File exists with 37 unit pairs

**Step 4: Commit**

```bash
git add data/units.txt
git commit -m "feat: add units.txt data file"
```

---

## Task 2: Create time_units.txt

**Files:**
- Create: `data/time_units.txt`

**Step 1: Create time_units.txt**

Create `data/time_units.txt`:

```txt
# Seconds
s = seconds
sec = seconds
secs = seconds
second = seconds
seconds = seconds

# Minutes
min = minutes
mins = minutes
minute = minutes
minutes = minutes

# Hours
h = hours
hr = hours
hrs = hours
hour = hours
hours = hours
```

**Step 2: Verify file created**

```bash
cat data/time_units.txt
```

Expected: File contains 14 time unit pairs

**Step 3: Commit**

```bash
git add data/time_units.txt
git commit -m "feat: add time_units.txt data file"
```

---

## Task 3: Create ingredients.txt

**Files:**
- Create: `data/ingredients.txt`

**Step 1: Create ingredients.txt**

Create `data/ingredients.txt`:

```txt
# Basics
salt
pepper
olive oil
vegetable oil
butter
water

# Aromatics
garlic
onion

# Broths
chicken broth
beef broth

# Baking
flour
sugar
eggs
milk
cream

# Dairy
cheese

# Produce
tomato
lemon
lime

# Herbs
parsley
cilantro
basil
oregano
thyme
rosemary

# Spices
cumin
paprika
cinnamon

# Flavor enhancers
vanilla
honey
soy sauce
vinegar
wine
```

**Step 2: Verify file created**

```bash
cat data/ingredients.txt | grep -v '^#' | grep -v '^$' | wc -l
```

Expected: 35 ingredients

**Step 3: Commit**

```bash
git add data/ingredients.txt
git commit -m "feat: add ingredients.txt data file"
```

---

## Task 4: Create cookware.txt

**Files:**
- Create: `data/cookware.txt`

**Step 1: Create cookware.txt**

Create `data/cookware.txt`:

```txt
# Cooking vessels
pot
pan
skillet
saucepan
wok
dutch oven
stockpot
frying pan

# Mixing bowls
bowl
mixing bowl
large bowl
small bowl

# Cutting tools
cutting board
knife
chef's knife
paring knife

# Heat sources
oven
stove
grill

# Powered appliances
blender
food processor
mixer
stand mixer

# Hand tools
whisk
spatula
wooden spoon
ladle
tongs

# Straining
colander
strainer
sieve

# Baking
baking sheet
baking dish
roasting pan
casserole dish

# Measuring
measuring cup
measuring spoons

# Other tools
rolling pin
grater
peeler
can opener
thermometer
timer

# Supplies
foil
parchment paper
plastic wrap
```

**Step 2: Verify file created**

```bash
cat data/cookware.txt | grep -v '^#' | grep -v '^$' | wc -l
```

Expected: 48 cookware items

**Step 3: Commit**

```bash
git add data/cookware.txt
git commit -m "feat: add cookware.txt data file"
```

---

## Task 5: Add parsing functions to completion.rs

**Files:**
- Modify: `src/completion.rs:1-143` (before get_completions function)

**Step 1: Add parsing functions after imports**

Add after the `use` statements and before the old constants (around line 9):

```rust
/// Parse unit pairs from embedded data (format: "short = long")
fn parse_unit_pairs(data: &str) -> Vec<(&'static str, &'static str)> {
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

/// Parse simple list from embedded data (one item per line)
fn parse_simple_list(data: &str) -> Vec<&'static str> {
    data.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}
```

**Step 2: Build to check for compilation errors**

```bash
cargo build
```

Expected: Build succeeds, warnings about unused functions are OK

**Step 3: Commit**

```bash
git add src/completion.rs
git commit -m "feat: add parsing functions for embedded data files"
```

---

## Task 6: Replace UNITS constant with LazyLock

**Files:**
- Modify: `src/completion.rs:10-37` (UNITS constant)

**Step 1: Add LazyLock import**

At the top of the file, add to imports:

```rust
use std::sync::LazyLock;
```

**Step 2: Replace UNITS constant**

Replace the entire UNITS constant (lines 10-37) with:

```rust
/// Common cooking units (loaded from embedded data/units.txt)
static UNITS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    parse_unit_pairs(include_str!("../data/units.txt"))
});
```

**Step 3: Build to verify**

```bash
cargo build
```

Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/completion.rs
git commit -m "refactor: convert UNITS to LazyLock with embedded data"
```

---

## Task 7: Replace TIME_UNITS constant with LazyLock

**Files:**
- Modify: `src/completion.rs:39-55` (TIME_UNITS constant)

**Step 1: Replace TIME_UNITS constant**

Replace the entire TIME_UNITS constant with:

```rust
/// Common time units (loaded from embedded data/time_units.txt)
static TIME_UNITS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    parse_unit_pairs(include_str!("../data/time_units.txt"))
});
```

**Step 2: Build to verify**

```bash
cargo build
```

Expected: Build succeeds

**Step 3: Commit**

```bash
git add src/completion.rs
git commit -m "refactor: convert TIME_UNITS to LazyLock with embedded data"
```

---

## Task 8: Replace COMMON_COOKWARE constant with LazyLock

**Files:**
- Modify: `src/completion.rs:57-105` (COMMON_COOKWARE constant)

**Step 1: Replace COMMON_COOKWARE constant**

Replace the entire COMMON_COOKWARE constant with:

```rust
/// Common cookware items (loaded from embedded data/cookware.txt)
static COMMON_COOKWARE: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    parse_simple_list(include_str!("../data/cookware.txt"))
});
```

**Step 2: Build to verify**

```bash
cargo build
```

Expected: Build succeeds

**Step 3: Commit**

```bash
git add src/completion.rs
git commit -m "refactor: convert COMMON_COOKWARE to LazyLock with embedded data"
```

---

## Task 9: Replace COMMON_INGREDIENTS constant with LazyLock

**Files:**
- Modify: `src/completion.rs:107-142` (COMMON_INGREDIENTS constant)

**Step 1: Replace COMMON_INGREDIENTS constant**

Replace the entire COMMON_INGREDIENTS constant with:

```rust
/// Common ingredients for suggestions (loaded from embedded data/ingredients.txt)
static COMMON_INGREDIENTS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    parse_simple_list(include_str!("../data/ingredients.txt"))
});
```

**Step 2: Build to verify final state**

```bash
cargo build
```

Expected: Build succeeds with no warnings

**Step 3: Commit**

```bash
git add src/completion.rs
git commit -m "refactor: convert COMMON_INGREDIENTS to LazyLock with embedded data"
```

---

## Task 10: Test completions functionality

**Files:**
- Test: Manual testing of completions

**Step 1: Build release binary**

```bash
cargo build --release
```

Expected: Build succeeds

**Step 2: Run language server**

Start the language server:

```bash
./target/release/cooklang-lsp
```

**Step 3: Test with VS Code extension**

If the VS Code extension is set up (from previous work):
1. Open a .cook file
2. Type `@` and verify ingredient completions appear
3. Type `#` and verify cookware completions appear
4. Type `~` and verify timer completions appear
5. Type `{1%` and verify unit completions appear

Expected: All completions work as before

**Step 4: Verify counts match**

Check that the number of items matches the old constants:
- Units: 37 items
- Time units: 14 items
- Ingredients: 35 items
- Cookware: 48 items

---

## Task 11: Final cleanup and documentation

**Files:**
- Modify: `README.md` (if it exists and mentions completion data)

**Step 1: Check if README needs updating**

```bash
grep -i "completion\|units\|ingredients" README.md
```

If README mentions how completions work or how to extend them, update it to reference the data/ directory.

**Step 2: Verify all data files are tracked**

```bash
git status
```

Expected: All data files are committed, working directory is clean

**Step 3: Create final summary commit if needed**

If README was updated:

```bash
git add README.md
git commit -m "docs: update README with data file locations"
```

---

## Verification Checklist

After completing all tasks, verify:

- [ ] `data/` directory exists with 4 text files
- [ ] `cargo build` succeeds with no warnings
- [ ] `cargo test` passes (if tests exist)
- [ ] Language server starts without errors
- [ ] Ingredient completions work (`@`)
- [ ] Cookware completions work (`#`)
- [ ] Timer completions work (`~`)
- [ ] Unit completions work (`%`)
- [ ] All commits are clean and well-messaged
- [ ] No hardcoded constant arrays remain in completion.rs

---

## Notes

- The `&'static str` lifetime is required because `include_str!` embeds the data as static strings
- `LazyLock` ensures parsing happens only once on first access
- Malformed lines in data files are silently skipped
- Comments starting with `#` are ignored
- Empty lines are ignored
- This approach has zero runtime overhead after first initialization
