# Lumina Plugin System

Lumina now supports secure JavaScript plugins executed via QuickJS in a Rust sandbox.

## How to use
1. Click the "Blocks" icon in the header.
2. Enter JavaScript code.
3. Click "Run Plugin".

## Available API
Currently, the following global functions are available:

- `print(msg)`: Prints a message to the backend console (and UI logs in future versions).

## Example: Math Calculation
```javascript
let a = 10;
let b = 20;
print("Calculating sum...");
a + b; // Returns 30
```

## Example: String Manipulation
```javascript
let name = "Lumina";
`Hello, ${name}! Current time is ${new Date().toString()}`;
```

## Future Plans
- `lumina.chat(prompt)`: Access AI from plugins.
- `lumina.fs.read(path)`: Secure file access.
