const fs = require('fs');
const path = require('path');
const ts = require(path.join(process.argv[2], 'node_modules/typescript'));
const root = path.resolve(process.argv[3]);

function visitFile(file) {
  const source = fs.readFileSync(file, 'utf8');
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  const symbols = [];
  function visit(node, owner) {
    if (ts.isClassDeclaration(node) && node.name) {
      const className = node.name.text;
      symbols.push({ file, line: tree.getLineAndCharacterOfPosition(node.getStart(tree)).line + 1, kind: 'class', name: className });
      for (const member of node.members) {
        if (ts.isMethodDeclaration(member) || ts.isGetAccessorDeclaration(member) || ts.isSetAccessorDeclaration(member)) {
          const name = member.name && member.name.getText(tree);
          if (name) symbols.push({ file, line: tree.getLineAndCharacterOfPosition(member.getStart(tree)).line + 1, kind: 'method', name: `${className}.${name}` });
        }
      }
    }
    if (ts.isFunctionDeclaration(node) && node.name) {
      symbols.push({ file, line: tree.getLineAndCharacterOfPosition(node.getStart(tree)).line + 1, kind: 'function', name: node.name.text });
    }
    if (ts.isVariableStatement(node)) {
      for (const declaration of node.declarationList.declarations) {
        if (declaration.initializer && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) {
          symbols.push({ file, line: tree.getLineAndCharacterOfPosition(declaration.getStart(tree)).line + 1, kind: 'function', name: declaration.name.getText(tree) });
        }
      }
    }
    ts.forEachChild(node, child => visit(child, owner));
  }
  visit(tree, undefined);
  return symbols;
}

function visitDirectory(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return visitDirectory(file);
    return entry.name.endsWith('.ts') ? visitFile(file) : [];
  });
}

for (const symbol of visitDirectory(root)) process.stdout.write(`${JSON.stringify(symbol)}\n`);
