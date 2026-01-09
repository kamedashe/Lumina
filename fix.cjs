const fs = require('fs');
const path = require('path');

// 1. Чиним tauri.conf.json
const tauriConfig = {
    "productName": "Lumina",
    "version": "0.1.0",
    "identifier": "com.lumina.app",
    "build": {
        "beforeDevCommand": "npm run dev",
        "beforeBuildCommand": "npm run build",
        "devUrl": "http://localhost:3000",
        "frontendDist": "../dist"
    },
    "app": {
        "windows": [
            {
                "title": "Lumina",
                "width": 900,
                "height": 680,
                "resizable": false,
                "decorations": false,
                "transparent": true,
                "alwaysOnTop": true,
                "center": true
            }
        ],
        "security": { "csp": null }
    },
    "bundle": {
        "active": true,
        "targets": "all",
        "icon": [
            "icons/32x32.png",
            "icons/128x128.png",
            "icons/128x128@2x.png",
            "icons/icon.icns",
            "icons/icon.ico"
        ]
    }
};

fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(tauriConfig, null, 2));
console.log('✅ tauri.conf.json пересоздан');

// 2. Чиним capabilities
const capDir = 'src-tauri/capabilities';
if (!fs.existsSync(capDir)) {
    fs.mkdirSync(capDir);
}

const capConfig = {
    "identifier": "default",
    "description": "Capability for the main window",
    "windows": ["*"],
    "permissions": ["core:default"]
};

fs.writeFileSync(path.join(capDir, 'default.json'), JSON.stringify(capConfig, null, 2));
console.log('✅ capabilities/default.json пересоздан');