#!/usr/bin/env python3
"""
Script to identify and help fix all unwrap()/panic!/expect() calls in the Supernova codebase
Following the Satoshi Standard: ZERO panics allowed
"""

import os
import re
import sys
from pathlib import Path
from typing import List, Tuple, Dict

class UnwrapFixer:
    def __init__(self, root_dir: str):
        self.root_dir = Path(root_dir)
        self.unwrap_pattern = re.compile(r'\.unwrap\(\)')
        self.expect_pattern = re.compile(r'\.expect\("([^"]*)"\)')
        self.panic_pattern = re.compile(r'panic!\(')
        self.test_pattern = re.compile(r'#\[cfg\(test\)\]|#\[test\]|mod tests')
        
    def find_rust_files(self) -> List[Path]:
        """Find all Rust files, excluding test files"""
        rust_files = []
        for file in self.root_dir.rglob("*.rs"):
            # Skip test files
            if "test" in file.name or "tests" in str(file.parent):
                continue
            rust_files.append(file)
        return rust_files
    
    def analyze_file(self, file_path: Path) -> Dict[str, List[Tuple[int, str]]]:
        """Analyze a file for unwrap/expect/panic usage"""
        issues = {
            'unwrap': [],
            'expect': [],
            'panic': []
        }
        
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                lines = f.readlines()
                
            in_test_block = False
            for i, line in enumerate(lines):
                # Check if we're in a test block
                if self.test_pattern.search(line):
                    in_test_block = True
                
                # Skip if in test
                if in_test_block and not line.strip().startswith("//"):
                    continue
                    
                # Check for unwrap()
                if self.unwrap_pattern.search(line):
                    issues['unwrap'].append((i + 1, line.strip()))
                
                # Check for expect()
                expect_match = self.expect_pattern.search(line)
                if expect_match:
                    issues['expect'].append((i + 1, line.strip()))
                
                # Check for panic!
                if self.panic_pattern.search(line):
                    issues['panic'].append((i + 1, line.strip()))
                    
        except Exception as e:
            print(f"Error reading {file_path}: {e}")
            
        return issues
    
    def generate_fix_suggestions(self, file_path: Path, issues: Dict) -> List[str]:
        """Generate fix suggestions for each issue"""
        suggestions = []
        
        for issue_type, occurrences in issues.items():
            for line_num, line_content in occurrences:
                if issue_type == 'unwrap':
                    suggestion = self.suggest_unwrap_fix(line_content)
                elif issue_type == 'expect':
                    suggestion = self.suggest_expect_fix(line_content)
                elif issue_type == 'panic':
                    suggestion = self.suggest_panic_fix(line_content)
                
                suggestions.append({
                    'file': str(file_path),
                    'line': line_num,
                    'type': issue_type,
                    'original': line_content,
                    'suggestion': suggestion
                })
                
        return suggestions
    
    def suggest_unwrap_fix(self, line: str) -> str:
        """Suggest a fix for unwrap() usage"""
        # Common patterns and their fixes
        if "lock().unwrap()" in line:
            return line.replace(".unwrap()", ".map_err(|e| StorageError::LockError(e.to_string()))?")
        elif "parse().unwrap()" in line:
            return line.replace(".unwrap()", ".map_err(|e| ValidationError::ParseError(e.to_string()))?")
        elif "to_string().unwrap()" in line:
            return line.replace(".unwrap()", ".map_err(|_| ValidationError::ConversionError)?")
        else:
            return line.replace(".unwrap()", "?")
    
    def suggest_expect_fix(self, line: str) -> str:
        """Suggest a fix for expect() usage"""
        match = self.expect_pattern.search(line)
        if match:
            error_msg = match.group(1)
            return line.replace(f'.expect("{error_msg}")', 
                              f'.ok_or_else(|| ValidationError::InvalidInput("{error_msg}".to_string()))?')
        return line
    
    def suggest_panic_fix(self, line: str) -> str:
        """Suggest a fix for panic! usage"""
        if "panic!(" in line:
            # Extract panic message if possible
            panic_match = re.search(r'panic!\("([^"]*)"\)', line)
            if panic_match:
                msg = panic_match.group(1)
                return f"return Err(SupernovaError::InternalError(\"{msg}\".to_string()));"
            else:
                return "return Err(SupernovaError::InternalError(\"Unexpected error\".to_string()));"
        return line
    
    def generate_report(self) -> None:
        """Generate a comprehensive report of all issues"""
        all_issues = []
        total_unwraps = 0
        total_expects = 0
        total_panics = 0
        
        rust_files = self.find_rust_files()
        print(f"Analyzing {len(rust_files)} Rust files...")
        
        for file_path in rust_files:
            issues = self.analyze_file(file_path)
            if any(issues.values()):
                suggestions = self.generate_fix_suggestions(file_path, issues)
                all_issues.extend(suggestions)
                
                total_unwraps += len(issues['unwrap'])
                total_expects += len(issues['expect'])
                total_panics += len(issues['panic'])
        
        # Print summary
        print("\n" + "="*80)
        print("UNWRAP/PANIC AUDIT REPORT")
        print("="*80)
        print(f"Total unwrap() calls: {total_unwraps}")
        print(f"Total expect() calls: {total_expects}")
        print(f"Total panic! calls: {total_panics}")
        print(f"TOTAL ISSUES: {total_unwraps + total_expects + total_panics}")
        print("="*80)
        
        # Group by file
        issues_by_file = {}
        for issue in all_issues:
            file = issue['file']
            if file not in issues_by_file:
                issues_by_file[file] = []
            issues_by_file[file].append(issue)
        
        # Print detailed report
        print("\nDETAILED FIXES NEEDED:")
        print("-"*80)
        
        for file, file_issues in sorted(issues_by_file.items()):
            print(f"\n{file}:")
            for issue in file_issues:
                print(f"  Line {issue['line']} ({issue['type']}):")
                print(f"    Original: {issue['original']}")
                print(f"    Fix:      {issue['suggestion']}")
        
        # Generate fix script
        self.generate_fix_script(issues_by_file)
    
    def generate_fix_script(self, issues_by_file: Dict) -> None:
        """Generate a shell script to apply fixes"""
        with open("apply_unwrap_fixes.sh", "w") as f:
            f.write("#!/bin/bash\n")
            f.write("# Script to apply unwrap fixes\n")
            f.write("# Review each change before running!\n\n")
            
            for file, file_issues in issues_by_file.items():
                f.write(f"\n# Fixes for {file}\n")
                for issue in file_issues:
                    # Generate sed command for simple replacements
                    if issue['type'] == 'unwrap':
                        f.write(f"# Line {issue['line']}: Replace unwrap()\n")
                        f.write(f"# sed -i '' 's/\\.unwrap()/\\?/g' {file}\n")
                    elif issue['type'] == 'expect':
                        f.write(f"# Line {issue['line']}: Replace expect()\n")
                        f.write(f"# Manual fix needed\n")
                    elif issue['type'] == 'panic':
                        f.write(f"# Line {issue['line']}: Replace panic!\n")
                        f.write(f"# Manual fix needed\n")
        
        print(f"\nFix script generated: apply_unwrap_fixes.sh")
        print("Review and edit the script before running!")

def main():
    if len(sys.argv) > 1:
        root_dir = sys.argv[1]
    else:
        root_dir = "."
    
    fixer = UnwrapFixer(root_dir)
    fixer.generate_report()

if __name__ == "__main__":
    main() 