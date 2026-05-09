package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

type FuncInfo struct {
	Name    string `json:"name"`
	Lines   int    `json:"lines"`
	Args    int    `json:"args"`
	Returns int    `json:"returns"`
}

type FileInfo struct {
	Path      string     `json:"path"`
	Lines     int        `json:"lines"`
	Funcs     []FuncInfo `json:"funcs"`
	FuncCount int        `json:"func_count"`
}

type CompareResult struct {
	GoPath   string   `json:"go_path"`
	RustPath string   `json:"rust_path"`
	IsDir    bool     `json:"is_dir"`
	GoFile   FileInfo `json:"go_file"`
	RustFile FileInfo `json:"rust_file"`
	Missing  []string `json:"missing_funcs"`
	Extra    []string `json:"extra_funcs"`
	LineDiff int      `json:"line_diff"`
}

func main() {
	if len(os.Args) < 3 {
		fmt.Println("Usage: go run compare_ast.go <go_dir> <rust_dir> [--json]")
		fmt.Println("Example: go run compare_ast.go /tmp/tork /home/lewis/src/twerk/crates")
		os.Exit(1)
	}

	goDir := os.Args[1]
	rustDir := os.Args[2]

	comparisons := buildComparisons(goDir, rustDir)

	isJson := len(os.Args) > 3 && os.Args[3] == "--json"

	if !isJson {
		fmt.Println("==============================================")
		fmt.Println("  GO TORK vs RUST TWERK - AST COMPARISON")
		fmt.Println("==============================================")
		fmt.Println()
	}

	totalGoLines := 0
	totalRustLines := 0
	totalGoFuncs := 0
	totalRustFuncs := 0

	for i := range comparisons {
		runComparison(&comparisons[i], goDir, rustDir)

		totalGoLines += comparisons[i].GoFile.Lines
		totalRustLines += comparisons[i].RustFile.Lines
		totalGoFuncs += comparisons[i].GoFile.FuncCount
		totalRustFuncs += comparisons[i].RustFile.FuncCount

		if !isJson {
			printComparison(&comparisons[i])
		}
	}

	if !isJson {
		fmt.Println("==============================================")
		fmt.Println("  SUMMARY")
		fmt.Println("==============================================")
		fmt.Println()
		fmt.Printf("Go total:    %d lines, %d functions\n", totalGoLines, totalGoFuncs)
		fmt.Printf("Rust total:  %d lines, %d functions\n", totalRustLines, totalRustFuncs)
		if totalGoLines > 0 {
			fmt.Printf("Line ratio:   %.2fx\n", float64(totalRustLines)/float64(totalGoLines))
		}
		if totalGoFuncs > 0 {
			fmt.Printf("Func ratio:   %.2fx\n", float64(totalRustFuncs)/float64(totalGoFuncs))
		}
		fmt.Println()
	}

	if isJson {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		enc.Encode(comparisons)
	}
}

func buildComparisons(goDir, rustDir string) []CompareResult {
	var comparisons []CompareResult
	seen := make(map[string]bool)

	// Walk Go directory recursively
	filepath.Walk(goDir, func(goPath string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}

		if info.IsDir() {
			return nil
		}

		// Skip test files
		if strings.HasSuffix(goPath, "_test.go") {
			return nil
		}

		relPath, err := filepath.Rel(goDir, goPath)
		if err != nil {
			return nil
		}

		// Skip vendor
		if strings.Contains(relPath, "vendor") {
			return nil
		}

		// Map Go path to Rust path
		rustPath := mapGoToRust(relPath)
		if rustPath == "" {
			return nil
		}

		key := relPath + ":" + rustPath
		if seen[key] {
			return nil
		}
		seen[key] = true

		comparisons = append(comparisons, CompareResult{
			GoPath:   relPath,
			RustPath: rustPath,
			IsDir:    false,
		})

		return nil
	})

	return comparisons
}

func mapGoToRust(relPath string) string {
	// Exact mappings - updated to match actual Rust project structure
	mappings := map[string]string{
		// Core types
		"job.go":   "twerk-core/src/job.rs",
		"task.go":  "twerk-core/src/task.rs",
		"node.go":  "twerk-core/src/node.rs",
		"user.go":  "twerk-core/src/user.rs",
		"mount.go": "twerk-core/src/mount.rs",
		"state.go": "twerk-core/src/state.rs",

		// Internal packages (core)
		"internal/eval/eval.go":                  "twerk-core/src/eval.rs",
		"internal/eval/funcs.go":                 "twerk-core/src/eval.rs",
		"internal/webhook/webhook.go":            "twerk-core/src/webhook.rs",
		"internal/uuid/uuid.go":                  "twerk-core/src/uuid.rs",
		"internal/encrypt/encrypt.go":            "twerk-core/src/encrypt.rs",
		"internal/redact/redact.go":              "twerk-core/src/redact.rs",
		"internal/fns/fns.go":                    "twerk-core/src/fns.rs",
		"internal/hash/hash.go":                  "twerk-core/src/hash.rs",
		"internal/host/host.go":                  "twerk-core/src/host.rs",
		"internal/wildcard/wildcard.go":          "twerk-common/src/wildcard.rs",
		"internal/slices/slices.go":              "twerk-common/src/slices.rs",
		"internal/httpx/httpx.go":                "twerk-infrastructure/src/httpx.rs",
		"internal/logging/logging.go":            "twerk-common/src/logging.rs",
		"internal/logging/writer.go":             "twerk-common/src/logging.rs",
		"internal/reexec/rexec.go":               "twerk-common/src/reexec.rs",
		"internal/reexec/command_linux.go":       "twerk-common/src/reexec.rs",
		"internal/reexec/command_unix.go":        "twerk-common/src/reexec.rs",
		"internal/reexec/command_unsupported.go": "twerk-common/src/reexec.rs",

		// Infrastructure - cache
		"internal/cache/cache.go": "twerk-infrastructure/src/cache/mod.rs",

		// Infrastructure - broker
		"broker/broker.go":   "twerk-infrastructure/src/broker/mod.rs",
		"broker/inmemory.go": "twerk-infrastructure/src/broker/inmemory.rs",

		// Infrastructure - locker
		"locker/locker.go":   "twerk-infrastructure/src/locker/mod.rs",
		"locker/inmemory.go": "twerk-infrastructure/src/locker/inmemory.rs",
		"locker/postgres.go": "twerk-infrastructure/src/locker/postgres.rs",

		// Coordinator
		"internal/coordinator/coordinator.go":         "twerk-app/src/engine/coordinator/mod.rs",
		"internal/coordinator/scheduler/scheduler.go": "twerk-app/src/engine/coordinator/scheduler.rs",
		"internal/coordinator/api/api.go":             "twerk-app/src/engine/coordinator/mod.rs",

		// Middleware
		"middleware/job/job.go":   "twerk-app/src/engine/coordinator/handlers.rs",
		"middleware/task/task.go": "twerk-app/src/engine/coordinator/handlers.rs",
		"middleware/node/node.go": "twerk-app/src/engine/coordinator/handlers.rs",
		"middleware/log/log.go":   "twerk-app/src/engine/coordinator/middleware.rs",

		// Runtime - shell (in twerk-app)
		"runtime/shell/shell.go": "twerk-app/src/engine/worker/shell.rs",
	}

	if val, ok := mappings[relPath]; ok {
		return val
	}

	// Handle Docker runtime directories
	if strings.HasPrefix(relPath, "runtime/docker/") {
		file := strings.TrimSuffix(strings.TrimPrefix(relPath, "runtime/docker/"), ".go")
		// Special case mappings for docker files (keys WITHOUT .go since file var has .go stripped)
		dockerMappings := map[string]string{
			"auth":       "twerk-infrastructure/src/runtime/docker/auth/mod.rs",
			"docker":     "twerk-infrastructure/src/runtime/docker/runtime.rs",
			"reference":  "twerk-infrastructure/src/runtime/docker/reference.rs",
			"tcontainer": "twerk-infrastructure/src/runtime/docker/container.rs",
			"archive":    "twerk-infrastructure/src/runtime/docker/archive.rs",
			"bind":       "twerk-infrastructure/src/runtime/docker/bind.rs",
			"config":     "twerk-infrastructure/src/runtime/docker/config.rs",
			"tmpfs":      "twerk-infrastructure/src/runtime/docker/tmpfs.rs",
			"volume":     "twerk-infrastructure/src/runtime/docker/volume.rs",
		}
		if val, ok := dockerMappings[file]; ok {
			return val
		}
		return fmt.Sprintf("twerk-infrastructure/src/runtime/docker/%s.rs", file)
	}

	// Handle Podman runtime directories
	if strings.HasPrefix(relPath, "runtime/podman/") {
		file := strings.TrimSuffix(strings.TrimPrefix(relPath, "runtime/podman/"), ".go")
		podmanMappings := map[string]string{
			"podman": "twerk-infrastructure/src/runtime/podman/runtime.rs",
			"volume": "twerk-infrastructure/src/runtime/podman/volume.rs",
		}
		if val, ok := podmanMappings[file]; ok {
			return val
		}
		return fmt.Sprintf("twerk-infrastructure/src/runtime/podman/%s.rs", file)
	}

	// Skip handlers directory (we map individual handler files)
	if strings.HasPrefix(relPath, "internal/coordinator/handlers/") {
		return "twerk-app/src/engine/coordinator/handlers.rs"
	}

	// Generic fallback for internal packages
	if strings.HasPrefix(relPath, "internal/") {
		base := strings.TrimSuffix(relPath, ".go")
		base = strings.ReplaceAll(base, "/", "_")
		return fmt.Sprintf("twerk-core/src/%s.rs", base)
	}

	return ""
}

// camelToSnake converts CamelCase to snake_case
// e.g., "NewJobSummary" -> "new_job_summary"
func camelToSnake(s string) string {
	var result strings.Builder
	for i, r := range s {
		if i > 0 && r >= 'A' && r <= 'Z' {
			// Check if it's a capital letter (not just first char)
			// Insert underscore before it
			prev := rune(s[i-1])
			if (prev >= 'a' && prev <= 'z') || (prev >= 'A' && prev <= 'Z') {
				result.WriteRune('_')
			}
		}
		result.WriteRune(r)
	}
	return strings.ToLower(result.String())
}

func runComparison(c *CompareResult, goDir, rustDir string) {
	goPath := filepath.Join(goDir, c.GoPath)
	rustPath := filepath.Join(rustDir, c.RustPath)

	// Check if Rust is a directory
	if fi, err := os.Stat(rustPath); err == nil && fi.IsDir() {
		c.IsDir = true
	}

	// Parse Go
	c.GoFile = parseGoFile(goPath)

	// Parse Rust
	if c.IsDir {
		c.RustFile = parseRustDir(rustPath)
	} else {
		c.RustFile = parseRustFile(rustPath)
	}

	// Find missing and extra functions (case-insensitive + CamelCase to snake_case normalization)
	goFuncs := make(map[string]bool)
	for _, f := range c.GoFile.Funcs {
		goFuncs[camelToSnake(f.Name)] = true
	}

	rustFuncs := make(map[string]bool)
	for _, f := range c.RustFile.Funcs {
		rustFuncs[strings.ToLower(f.Name)] = true
	}

	for _, f := range c.GoFile.Funcs {
		goName := camelToSnake(f.Name)
		if !rustFuncs[goName] {
			c.Missing = append(c.Missing, f.Name)
		}
	}

	for _, f := range c.RustFile.Funcs {
		rustName := strings.ToLower(f.Name)
		if !goFuncs[rustName] {
			c.Extra = append(c.Extra, f.Name)
		}
	}

	c.LineDiff = c.GoFile.Lines - c.RustFile.Lines
}

func printComparison(c *CompareResult) {
	status := "✅"
	if len(c.Missing) > 0 && len(c.Missing) <= 3 {
		status = "⚠️ "
	} else if len(c.Missing) > 3 {
		status = "❌ "
	}

	fmt.Printf("%s %s\n", status, c.GoPath)
	fmt.Printf("   Go:      %4d lines, %3d functions\n", c.GoFile.Lines, c.GoFile.FuncCount)
	fmt.Printf("   Rust:    %4d lines, %3d functions\n", c.RustFile.Lines, c.RustFile.FuncCount)
	fmt.Printf("   Diff:    %+d lines\n", c.LineDiff)

	if len(c.Missing) > 0 {
		fmt.Printf("   Missing: %d functions", len(c.Missing))
		if len(c.Missing) <= 10 {
			fmt.Println()
			for _, m := range c.Missing {
				fmt.Printf("      - %s\n", m)
			}
		} else {
			fmt.Printf(" (%d shown)\n", 10)
			for i := 0; i < 10 && i < len(c.Missing); i++ {
				fmt.Printf("      - %s\n", c.Missing[i])
			}
		}
	}

	if len(c.Extra) > 0 && len(c.Extra) <= 5 {
		fmt.Printf("   Extra Rust: %d functions\n", len(c.Extra))
		for _, e := range c.Extra {
			fmt.Printf("      + %s\n", e)
		}
	}

	fmt.Println()
}

func parseGoFile(path string) FileInfo {
	info := FileInfo{Path: path}

	data, err := os.ReadFile(path)
	if err != nil {
		return info
	}

	info.Lines = strings.Count(string(data), "\n") + 1

	// Parse using AST
	fset := token.NewFileSet()
	node, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
	if err != nil {
		return info
	}

	// Extract function info
	for _, decl := range node.Decls {
		if fn, ok := decl.(*ast.FuncDecl); ok {
			// Skip methods (they have receivers)
			if fn.Recv != nil {
				continue
			}
			funcInfo := FuncInfo{Name: fn.Name.Name}
			if fn.Pos().IsValid() && fn.End().IsValid() {
				funcInfo.Lines = int(fset.Position(fn.End()).Line - fset.Position(fn.Pos()).Line + 1)
			}
			if fn.Type.Params != nil {
				funcInfo.Args = len(fn.Type.Params.List)
			}
			if fn.Type.Results != nil {
				funcInfo.Returns = len(fn.Type.Results.List)
			}
			info.Funcs = append(info.Funcs, funcInfo)
		}
	}

	info.FuncCount = len(info.Funcs)
	return info
}

func parseRustFile(path string) FileInfo {
	info := FileInfo{Path: path}

	data, err := os.ReadFile(path)
	if err != nil {
		return info
	}

	info.Lines = strings.Count(string(data), "\n") + 1

	// Find all function definitions (skipping macros and attributes)
	// Match: pub fn name or fn name (not at start of line after #)
	fnRegex := regexp.MustCompile(`(?m)^#\[.*\]\s*((pub\s+)?fn\s+\w+)`)
	matches := fnRegex.FindAllStringSubmatchIndex(string(data), -1)

	type fnMatch struct {
		name  string
		start int
		end   int
	}

	var fnMatches []fnMatch
	for i, m := range matches {
		if len(m) >= 4 {
			// Get the full match group
			fullMatch := string(data)[m[0]:m[1]]
			// Extract function name
			nameRegex := regexp.MustCompile(`fn\s+(\w+)`)
			nameMatch := nameRegex.FindStringSubmatch(fullMatch)
			if len(nameMatch) >= 2 {
				name := nameMatch[1]
				startLine := strings.Count(string(data)[:m[0]], "\n") + 1
				var endLine int
				if i+1 < len(matches) {
					endLine = strings.Count(string(data)[:matches[i+1][0]], "\n")
				} else {
					endLine = info.Lines
				}
				fnMatches = append(fnMatches, fnMatch{name, startLine, endLine})
			}
		}
	}

	for _, fm := range fnMatches {
		info.Funcs = append(info.Funcs, FuncInfo{
			Name:  fm.name,
			Lines: fm.end - fm.start + 1,
		})
	}

	info.FuncCount = len(info.Funcs)
	return info
}

func parseRustDir(dir string) FileInfo {
	info := FileInfo{Path: dir}
	var allFuncs []FuncInfo
	var totalLines int

	filepath.Walk(dir, func(path string, fi os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if filepath.Ext(path) == ".rs" {
			if data, err := os.ReadFile(path); err == nil {
				totalLines += strings.Count(string(data), "\n") + 1

				// Find functions
				fnRegex := regexp.MustCompile(`(?m)^(?:#\[.*\]\s*)*(pub\s+)?fn\s+(\w+)`)
				matches := fnRegex.FindAllStringSubmatchIndex(string(data), -1)

				for i, m := range matches {
					if len(m) >= 6 {
						name := string(data)[m[4]:m[5]]
						startLine := strings.Count(string(data)[:m[0]], "\n") + 1
						var endLine int
						if i+1 < len(matches) {
							endLine = strings.Count(string(data)[:matches[i+1][0]], "\n")
						} else {
							endLine = strings.Count(string(data), "\n") + 1
						}
						allFuncs = append(allFuncs, FuncInfo{
							Name:  name,
							Lines: endLine - startLine + 1,
						})
					}
				}
			}
		}
		return nil
	})

	info.Lines = totalLines
	info.Funcs = allFuncs
	info.FuncCount = len(allFuncs)
	return info
}
