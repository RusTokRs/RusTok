const fs = require('fs');
const path = require('path');

function walkDir(dir, callback) {
  fs.readdirSync(dir).forEach((f) => {
    let dirPath = path.join(dir, f);
    let isDirectory = fs.statSync(dirPath).isDirectory();
    if (isDirectory) {
      walkDir(dirPath, callback);
    } else if (f.endsWith('.ts') || f.endsWith('.tsx')) {
      callback(path.join(dir, f));
    }
  });
}

const targetDir = process.cwd();

walkDir(targetDir, (filePath) => {
  let content = fs.readFileSync(filePath, 'utf8');
  let original = content;

  // Replace layout and navigation components
  content = content.replace(
    /import\s+\{([^}]+)\}\s+from\s+'(?:@\/)?components\/(?:layout|nav-[^']+)'/g,
    "import {} from '@/widgets/app-shell'"
  );
  content = content.replace(
    /import\s+([A-Za-z0-9_]+)\s+from\s+'(?:@\/)?components\/layout\/([^']+)'/g,
    "import  from '@/widgets/app-shell'"
  );
  content = content.replace(
    /import\s+([A-Za-z0-9_]+)\s+from\s+'(?:@\/)?components\/nav-[^']+'/g,
    "import  from '@/widgets/app-shell'"
  );

  // Replace kbar
  content = content.replace(
    /'(?:@\/)?components\/kbar(?:\/[^']*)?'/g,
    "'@/widgets/command-palette'"
  );

  // Replace table components
  content = content.replace(
    /'(?:@\/)?components\/ui\/table(?:\/[^']*)?'/g,
    "'@/widgets/data-table'"
  );
  content = content.replace(
    /'(?:@\/)?shared\/ui\/shadcn\/table(?:\/[^']*)?'/g,
    "'@/widgets/data-table'"
  );

  // Replace hooks
  content = content.replace(/'(?:@\/)?hooks([^']+)'/g, "'@/shared/hooks'");

  // Replace lib
  content = content.replace(
    /'(?:@\/)?lib\/graphql'/g,
    "'@/shared/api/graphql'"
  );
  content = content.replace(
    /'(?:@\/)?lib\/auth-api'/g,
    "'@/shared/api/auth-api'"
  );
  content = content.replace(/'(?:@\/)?lib([^']+)'/g, "'@/shared/lib'");

  // Replace themes
  content = content.replace(
    /'(?:@\/)?components\/themes([^']+)'/g,
    "'@/shared/lib/themes'"
  );

  // Replace constants
  content = content.replace(
    /'(?:@\/)?constants([^']+)'/g,
    "'@/shared/constants'"
  );

  // Replace ui (forms, alert modal, etc)
  content = content.replace(
    /'(?:@\/)?components\/forms(?:\/[^']*)?'/g,
    "'@/shared/ui/forms'"
  );
  content = content.replace(
    /'(?:@\/)?components\/modal\/alert-modal'/g,
    "'@/widgets/alert-modal'"
  );
  content = content.replace(
    /'(?:@\/)?components\/ui([^']+)'/g,
    "'@/shared/ui/shadcn'"
  );
  content = content.replace(
    /'(?:@\/)?components\/icons'/g,
    "'@/shared/ui/icons'"
  );
  content = content.replace(
    /import\s+\{([^}]+)\}\s+from\s+'(?:@\/)?components\/(breadcrumbs|file-uploader|search-input|form-card-skeleton)'/g,
    "import {} from '@/shared/ui'"
  );

  if (content !== original) {
    fs.writeFileSync(filePath, content, 'utf8');
    console.log('Updated: ' + filePath);
  }
});
