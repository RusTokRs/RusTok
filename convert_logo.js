const sharp = require('sharp');

async function convert() {
    console.log("Starting conversion...");
    const input = 'assets/rustok-logo.png';
    
    // SVG is vectors, can't easily auto-convert from PNG accurately with sharp,
    // but we can generate WebP, AVIF, and different sizes of PNG.
    
    // 1. WebP
    await sharp(input)
        .webp({ quality: 80 })
        .toFile('assets/rustok-logo.webp');
    console.log("Created rustok-logo.webp");

    // 2. AVIF
    await sharp(input)
        .avif({ quality: 80 })
        .toFile('assets/rustok-logo.avif');
    console.log("Created rustok-logo.avif");

    // 3. Small PNG (e.g. for badges or tiny icons)
    await sharp(input)
        .resize(32, 32)
        .toFile('assets/rustok-logo-32x32.png');
    console.log("Created rustok-logo-32x32.png");

    // 4. ICO (we can just cheat and rename a 32x32 png or use another tool, 
    // but sharp doesn't natively output .ico. Let's just create standard favicon sizes)
    await sharp(input)
        .resize(16, 16)
        .toFile('assets/rustok-logo-16x16.png');
    console.log("Created rustok-logo-16x16.png");
    
    await sharp(input)
        .resize(192, 192)
        .toFile('assets/rustok-logo-192x192.png');
    console.log("Created rustok-logo-192x192.png");
    
    await sharp(input)
        .resize(512, 512)
        .toFile('assets/rustok-logo-512x512.png');
    console.log("Created rustok-logo-512x512.png");

    console.log("Done converting!");
}

convert().catch(console.error);
