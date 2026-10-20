.pragma library
// Generated from pinned T3 Code fb990765a49414a5bfbdc73365455fc8b5fbe691. MIT, see licenses/T3-Code-MIT.txt.
// Regenerate: node tools/build-keybindings.cjs <native-checkout>
var nativeKeys=(function(){
"use strict";
const isElectron=true;
const scriptIdLimit=24;
const SCRIPT_RUN_COMMAND_PATTERN={parts:[{literal:"script."},null,{literal:".run"}]};
function isScriptRunCommand(command){const id=command.slice(7,-4);return command.startsWith("script.")&&command.endsWith(".run")&&id.length>0&&id.length<=scriptIdLimit&&/^[a-z0-9][a-z0-9-]*$/.test(id); }
function flattenNativeGroups(groups,mapper){return [].concat.apply([],groups.map(mapper));}
const MAX_KEYBINDING_VALUE_LENGTH = 64;
const MAX_WHEN_EXPRESSION_DEPTH = 64;
const MAX_SCRIPT_ID_LENGTH = 24;
const MAX_KEYBINDINGS_COUNT = 256;
const THREAD_JUMP_KEYBINDING_COMMANDS = [
  "thread.jump.1",
  "thread.jump.2",
  "thread.jump.3",
  "thread.jump.4",
  "thread.jump.5",
  "thread.jump.6",
  "thread.jump.7",
  "thread.jump.8",
  "thread.jump.9",
]         ;
const MODEL_PICKER_JUMP_KEYBINDING_COMMANDS = [
  "modelPicker.jump.1",
  "modelPicker.jump.2",
  "modelPicker.jump.3",
  "modelPicker.jump.4",
  "modelPicker.jump.5",
  "modelPicker.jump.6",
  "modelPicker.jump.7",
  "modelPicker.jump.8",
  "modelPicker.jump.9",
]         ;
const THREAD_KEYBINDING_COMMANDS = [
  "thread.stop",
  "thread.steerQueuedMessage",
  "thread.editQueuedMessage",
  "thread.previous",
  "thread.next",
  "thread.copyReference",
  "thread.settle",
  "thread.pin",
  "thread.undo",
  ...THREAD_JUMP_KEYBINDING_COMMANDS,
]         ;
const MODEL_PICKER_KEYBINDING_COMMANDS = [
  "modelPicker.toggle",
  "modelPicker.previousProvider",
  "modelPicker.nextProvider",
  ...MODEL_PICKER_JUMP_KEYBINDING_COMMANDS,
]         ;
const STATIC_KEYBINDING_COMMANDS = [
  "sidebar.toggle",
  "navigation.back",
  "navigation.forward",
  "terminal.toggle",
  "terminal.split",
  "terminal.splitVertical",
  "terminal.new",
  "terminal.close",
  "rightPanel.toggle",
  "rightPanel.new",
  "threadPanel.toggle",
  "rightPanel.toggleMaximized",
  "rightPanel.close",
  "view.reopenClosed",
  "pullRequest.copyNumber",
  "diff.toggle",
  "preview.toggle",
  "preview.refresh",
  "preview.focusUrl",
  "preview.zoomIn",
  "preview.zoomOut",
  "preview.resetZoom",
  "commandPalette.toggle",
  "filePicker.toggle",
  "projectSearch.toggle",
  "usage.open",
  "theme.select",
  "appearance.cycle",
  "themeEditor.toggle",
  "composer.stash",
  "composer.sendAlternate",
  "composer.sendBackground",
  "composer.sendAndNewThread",
  "composer.host",
  "composer.cycleHost",
  "composer.effort",
  "composer.increaseEffort",
  "composer.decreaseEffort",
  "composer.mode",
  "composer.workspace",
  "composer.previousWorktree",
  "composer.branch",
  "chat.new",
  "chat.newLocal",
  "chat.newWithoutProject",
  "chat.find",
  "editor.openFavorite",
  "usage.cost",
  "usage.tokens",
  "usage.limits",
  "usage.period.day",
  "usage.period.week",
  "usage.period.month",
  "usage.period.quarter",
  ...MODEL_PICKER_KEYBINDING_COMMANDS,
  ...THREAD_KEYBINDING_COMMANDS,
]         ;

const EVENT_CODE_SHORTCUT_KEYS                                   = {
  Backquote: "`",
  Backslash: "\\",
  BracketLeft: "[",
  BracketRight: "]",
  Comma: ",",
  Digit0: "0",
  Digit1: "1",
  Digit2: "2",
  Digit3: "3",
  Digit4: "4",
  Digit5: "5",
  Digit6: "6",
  Digit7: "7",
  Digit8: "8",
  Digit9: "9",
  Equal: "=",
  Minus: "-",
  Period: ".",
  Quote: "'",
  Semicolon: ";",
  Slash: "/",
};

const US_SHIFTED_TO_UNSHIFTED_KEYS                              = new Map([
  ["!", "1"],
  ["@", "2"],
  ["#", "3"],
  ["$", "4"],
  ["%", "5"],
  ["^", "6"],
  ["&", "7"],
  ["*", "8"],
  ["(", "9"],
  [")", "0"],
  ["~", "`"],
  ["_", "-"],
  ["+", "="],
  ["{", "["],
  ["}", "]"],
  ["|", "\\"],
  [":", ";"],
  ['"', "'"],
  ["<", ","],
  [">", "."],
  ["?", "/"],
]);

function isMacPlatform(platform        )          {
  return /mac|iphone|ipad|ipod/i.test(platform);
}

function normalizeEventKey(key        )         {
  const normalized = key.toLowerCase();
  if (normalized === "esc") return "escape";
  return normalized;
}

function shortcutKeyFromEvent(event                                         )         {
  const layoutKey = normalizeEventKey(event.key);
  if (/^[a-z]$/.test(layoutKey)) return layoutKey;
  const physicalKey = event.code ? EVENT_CODE_SHORTCUT_KEYS[event.code] : undefined;
  return physicalKey ?? layoutKey;
}

function resolveEventKeys(event                   )              {
  const layoutKey = normalizeEventKey(event.key);
  const keys = new Set([layoutKey]);
  const letterCode = event.code?.match(/^Key([A-Z])$/)?.[1];
  if (letterCode && !/^[a-z]$/.test(layoutKey)) {
    keys.add(letterCode.toLowerCase());
  }
  keys.add(shortcutKeyFromEvent(event));
  return keys;
}

function matchesKeybindingShortcutModifiers(
  event                           ,
  shortcut                    ,
  platform        ,
)          {
  const useMetaForMod = isMacPlatform(platform);
  const expectedMeta = shortcut.metaKey || (shortcut.modKey && useMetaForMod);
  const expectedCtrl = shortcut.ctrlKey || (shortcut.modKey && !useMetaForMod);
  return (
    event.metaKey === expectedMeta &&
    event.ctrlKey === expectedCtrl &&
    event.shiftKey === shortcut.shiftKey &&
    event.altKey === shortcut.altKey
  );
}

function matchesKeybindingShortcut(
  event                   ,
  shortcut                    ,
  platform        ,
)          {
  if (
    !isMacPlatform(platform) &&
    event.getModifierState?.("AltGraph") &&
    !/^[a-z0-9]$/i.test(event.key)
  )
    return false;
  if (!matchesKeybindingShortcutModifiers(event, shortcut, platform)) return false;
  return resolveEventKeys(event).has(shortcut.key);
}

                
                                         
                   
                   
                  
                      
                       

const DEFAULT_KEYBINDINGS                                = [
  { key: "mod+b", command: "sidebar.toggle" },
  { key: "mod+[", command: "navigation.back", when: "!terminalFocus" },
  { key: "mod+]", command: "navigation.forward", when: "!terminalFocus" },
  { key: "mod+j", command: "terminal.toggle" },
  { key: "mod+alt+b", command: "rightPanel.toggle" },
  { key: "mod+shift+b", command: "threadPanel.toggle", when: "!terminalFocus" },
  { key: "mod+t", command: "rightPanel.new", when: "rightPanelOpen && !terminalFocus" },
  { key: "mod+d", command: "terminal.split", when: "terminalFocus" },
  { key: "mod+shift+d", command: "terminal.splitVertical", when: "terminalFocus" },
  { key: "mod+n", command: "terminal.new", when: "terminalFocus" },
  { key: "mod+w", command: "terminal.close", when: "terminalFocus" },
  { key: "mod+w", command: "rightPanel.close", when: "!terminalFocus" },
  { key: "mod+shift+t", command: "view.reopenClosed" },
  { key: "mod+d", command: "diff.toggle", when: "!terminalFocus" },
  { key: "mod+shift+j", command: "preview.toggle" },
  { key: "mod+r", command: "preview.refresh", when: "previewFocus" },
  { key: "mod+l", command: "preview.focusUrl", when: "previewFocus" },
  { key: "mod+=", command: "preview.zoomIn", when: "previewFocus" },
  { key: "mod++", command: "preview.zoomIn", when: "previewFocus" },
  { key: "mod+-", command: "preview.zoomOut", when: "previewFocus" },
  { key: "mod+0", command: "preview.resetZoom", when: "previewFocus" },
  { key: "mod+k", command: "commandPalette.toggle", when: "!terminalFocus" },
  { key: "mod+p", command: "filePicker.toggle", when: "!terminalFocus" },
  { key: "mod+shift+f", command: "projectSearch.toggle", when: "!terminalFocus" },
  { key: "mod+u", command: "usage.open", when: "!terminalFocus" },
  { key: "mod+alt+a", command: "theme.select", when: "!terminalFocus" },
  { key: "mod+alt+shift+a", command: "appearance.cycle", when: "!terminalFocus" },
  { key: "mod+alt+shift+t", command: "themeEditor.toggle" },
  { key: "mod+s", command: "composer.stash", when: "!terminalFocus" },
  { key: "mod+shift+enter", command: "thread.steerQueuedMessage", when: "!terminalFocus" },
  { key: "alt+arrowup", command: "thread.editQueuedMessage", when: "composerFocus" },
  { key: "mod+enter", command: "composer.sendAlternate", when: "composerFocus && turnRunning" },
  {
    key: "mod+enter",
    command: "composer.sendBackground",
    when: "composerFocus && draftThreadRoute",
  },
  {
    key: "mod+alt+enter",
    command: "composer.sendBackground",
    when: "composerFocus && draftThreadRoute",
  },
  {
    key: "mod+alt+enter",
    command: "composer.sendAndNewThread",
    when: "composerFocus && !draftThreadRoute",
  },
  { key: "mod+n", command: "chat.new", when: "!terminalFocus" },
  { key: "mod+shift+o", command: "chat.new", when: "!terminalFocus" },
  { key: "mod+shift+n", command: "chat.newLocal", when: "!terminalFocus" },
  { key: "mod+alt+n", command: "chat.newWithoutProject", when: "!terminalFocus" },
  { key: "mod+f", command: "chat.find", when: "!terminalFocus && !previewFocus" },
  { key: "mod+shift+m", command: "modelPicker.toggle", when: "!terminalFocus" },
  { key: "mod+shift+h", command: "composer.host", when: "!terminalFocus" },
  { key: "mod+shift+e", command: "composer.effort", when: "!terminalFocus" },
  { key: "mod+shift+a", command: "composer.mode", when: "!terminalFocus" },
  { key: "mod+shift+x", command: "composer.workspace", when: "!terminalFocus" },
  { key: "mod+shift+g", command: "composer.branch", when: "!terminalFocus" },
  { key: "mod+shift+l", command: "composer.previousWorktree", when: "!terminalFocus" },
  { key: "mod+shift+k", command: "pullRequest.copyNumber", when: "!terminalFocus" },
  { key: "mod+shift+arrowup", command: "modelPicker.previousProvider", when: "modelPickerOpen" },
  { key: "mod+shift+arrowdown", command: "modelPicker.nextProvider", when: "modelPickerOpen" },
  { key: "mod+o", command: "editor.openFavorite" },
  { key: "mod+shift+[", command: "thread.previous" },
  { key: "mod+shift+]", command: "thread.next" },
  { key: "mod+shift+c", command: "thread.copyReference", when: "!terminalFocus" },
  { key: "mod+shift+s", command: "thread.settle", when: "!terminalFocus" },
  { key: "mod+shift+p", command: "thread.pin", when: "!terminalFocus" },
  { key: "mod+z", command: "thread.undo", when: "!terminalFocus && !editableFocus" },
  ...THREAD_JUMP_KEYBINDING_COMMANDS.map((command, index) => ({
    key: `mod+${index + 1}`,
    command,
    when: "!modelPickerOpen && isDesktop",
  })),
  ...MODEL_PICKER_JUMP_KEYBINDING_COMMANDS.map((command, index) => ({
    key: `mod+${index + 1}`,
    command,
    when: "modelPickerOpen && isDesktop",
  })),
  { key: "c", command: "usage.cost", when: "usagePageOpen" },
  { key: "t", command: "usage.tokens", when: "usagePageOpen" },
  { key: "l", command: "usage.limits", when: "usagePageOpen" },
  { key: "mod+shift+1", command: "usage.period.day", when: "usagePageOpen" },
  { key: "mod+shift+2", command: "usage.period.week", when: "usagePageOpen" },
  { key: "mod+shift+3", command: "usage.period.month", when: "usagePageOpen" },
  { key: "mod+shift+4", command: "usage.period.quarter", when: "usagePageOpen" },
];

function normalizeKeyToken(token        )         {
  if (token === "space") return " ";
  if (token === "esc") return "escape";
  return token;
}

function parseKeybindingShortcut(value        )                            {
  const rawTokens = value
    .toLowerCase()
    .split("+")
    .map((token) => token.trim());
  const tokens = [...rawTokens];
  let trailingEmptyCount = 0;
  while (tokens[tokens.length - 1] === "") {
    trailingEmptyCount += 1;
    tokens.pop();
  }
  if (trailingEmptyCount > 0) {
    tokens.push("+");
  }
  if (tokens.some((token) => token.length === 0)) {
    return null;
  }
  if (tokens.length === 0) return null;

  let key                = null;
  let metaKey = false;
  let ctrlKey = false;
  let shiftKey = false;
  let altKey = false;
  let modKey = false;

  for (const token of tokens) {
    switch (token) {
      case "cmd":
      case "meta":
        metaKey = true;
        break;
      case "ctrl":
      case "control":
        ctrlKey = true;
        break;
      case "shift":
        shiftKey = true;
        break;
      case "alt":
      case "option":
        altKey = true;
        break;
      case "mod":
        modKey = true;
        break;
      default: {
        if (key !== null) return null;
        key = normalizeKeyToken(token);
      }
    }
  }

  if (key === null) return null;
  return {
    key,
    metaKey,
    ctrlKey,
    shiftKey,
    altKey,
    modKey,
  };
}

function tokenizeWhenExpression(expression        )                     {
  const tokens              = [];
  let index = 0;

  while (index < expression.length) {
    const current = expression[index];
    if (!current) break;

    if (/\s/.test(current)) {
      index += 1;
      continue;
    }
    if (expression.startsWith("&&", index)) {
      tokens.push({ type: "and" });
      index += 2;
      continue;
    }
    if (expression.startsWith("||", index)) {
      tokens.push({ type: "or" });
      index += 2;
      continue;
    }
    if (current === "!") {
      tokens.push({ type: "not" });
      index += 1;
      continue;
    }
    if (current === "(") {
      tokens.push({ type: "lparen" });
      index += 1;
      continue;
    }
    if (current === ")") {
      tokens.push({ type: "rparen" });
      index += 1;
      continue;
    }

    const identifier = /^[A-Za-z_][A-Za-z0-9_.-]*/.exec(expression.slice(index));
    if (!identifier) {
      return null;
    }
    tokens.push({ type: "identifier", value: identifier[0] });
    index += identifier[0].length;
  }

  return tokens;
}

function parseKeybindingWhenExpression(expression        )                            {
  const tokens = tokenizeWhenExpression(expression);
  if (!tokens || tokens.length === 0) return null;
  let index = 0;

  let parseOr;
  const parsePrimary = (depth        )                            => {
    if (depth > MAX_WHEN_EXPRESSION_DEPTH) {
      return null;
    }
    const token = tokens[index];
    if (!token) return null;

    if (token.type === "identifier") {
      index += 1;
      return { type: "identifier", name: token.value };
    }

    if (token.type === "lparen") {
      index += 1;
      const expressionNode = parseOr(depth + 1);
      const closeToken = tokens[index];
      if (!expressionNode || !closeToken || closeToken.type !== "rparen") {
        return null;
      }
      index += 1;
      return expressionNode;
    }

    return null;
  };

  const parseUnary = (depth        )                            => {
    let notCount = 0;
    while (tokens[index]?.type === "not") {
      index += 1;
      notCount += 1;
      if (notCount > MAX_WHEN_EXPRESSION_DEPTH) {
        return null;
      }
    }

    let node = parsePrimary(depth);
    if (!node) return null;

    while (notCount > 0) {
      node = { type: "not", node };
      notCount -= 1;
    }

    return node;
  };

  const parseAnd = (depth        )                            => {
    let left = parseUnary(depth);
    if (!left) return null;

    while (tokens[index]?.type === "and") {
      index += 1;
      const right = parseUnary(depth);
      if (!right) return null;
      left = { type: "and", left, right };
    }

    return left;
  };

  parseOr = (depth        )                            => {
    let left = parseAnd(depth);
    if (!left) return null;

    while (tokens[index]?.type === "or") {
      index += 1;
      const right = parseAnd(depth);
      if (!right) return null;
      left = { type: "or", left, right };
    }

    return left;
  };

  const ast = parseOr(0);
  if (!ast || index !== tokens.length) return null;
  return ast;
}

function compileResolvedKeybindingRule(rule                )                                {
  const shortcut = parseKeybindingShortcut(rule.key);
  if (!shortcut) return null;

  if (rule.when !== undefined) {
    const whenAst = parseKeybindingWhenExpression(rule.when);
    if (!whenAst) return null;
    return {
      command: rule.command,
      shortcut,
      whenAst,
    };
  }

  return {
    command: rule.command,
    shortcut,
  };
}

function compileResolvedKeybindingsConfig(
  config                               ,
)                            {
  const compiled                           = [];
  for (const rule of config) {
    const result = compileResolvedKeybindingRule(rule);
    if (result) {
      compiled.push(result);
    }
  }
  return compiled.slice(-MAX_KEYBINDINGS_COUNT);
}

const DEFAULT_RESOLVED_KEYBINDINGS = compileResolvedKeybindingsConfig(DEFAULT_KEYBINDINGS);

function mergeWithDefaultKeybindings(
  custom                           ,
)                            {
  if (custom.length === 0) {
    return [...DEFAULT_RESOLVED_KEYBINDINGS];
  }

  const overriddenCommands = new Set(custom.map((binding) => binding.command));
  const retainedDefaults = DEFAULT_RESOLVED_KEYBINDINGS.filter(
    (binding) => !overriddenCommands.has(binding.command),
  );
  const merged = [...retainedDefaults, ...custom];

  if (merged.length <= MAX_KEYBINDINGS_COUNT) {
    return merged;
  }

  return merged.slice(-MAX_KEYBINDINGS_COUNT);
}

                                                                                       
                                                             

const METRIC_OPTIONS = [
  { value: "cost", label: "Cost", command: "usage.cost" },
  { value: "tokens", label: "Tokens", command: "usage.tokens" },
  { value: "limits", label: "Limits", command: "usage.limits" },
]                                                                                                ;

const WINDOW_OPTIONS = [
  { days: 1, label: "Past 24h", command: "usage.period.day" },
  { days: 7, label: "7 days", command: "usage.period.week" },
  { days: 30, label: "30 days", command: "usage.period.month" },
  { days: 90, label: "90 days", command: "usage.period.quarter" },
]         ;

function resolveUsageShortcut(
  event                                                    ,
  keybindings                           ,
) {
  const target = event.target;
  if (
    target instanceof HTMLElement &&
    (target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.tagName === "SELECT" ||
      target.isContentEditable ||
      target.closest('[role="dialog"], [aria-modal="true"], [data-slot$="popup"]'))
  ) {
    return null;
  }

  return resolveShortcutCommand(event, keybindings, {
    context: { usagePageOpen: true },
  });
}





// Every usage.* command needs a rank. An unranked one falls back to the
// alphabetical compare, which makes the comparator inconsistent and the order
// depend on the input order.
const usageCommandOrder = new Map                           (
  [
    "usage.open"         ,
    ...[...METRIC_OPTIONS, ...WINDOW_OPTIONS].map((option) => option.command),
  ].map((command, index) => [command, index]),
);

const firstUsageCommand = METRIC_OPTIONS[0].command;

function compareCommands(
  left                   ,
  right                   ,
  key                                        ,
)         {
  const leftRank = usageCommandOrder.get(left);
  const rightRank = usageCommandOrder.get(right);
  if (leftRank !== undefined && rightRank !== undefined) return leftRank - rightRank;
  return key(leftRank === undefined ? left : firstUsageCommand).localeCompare(
    key(rightRank === undefined ? right : firstUsageCommand),
  );
}

                                                                

                                
                      
                                      
                       
                        
                                    
                                     
                               
                                           
                                            
 

                                        
                                                        

const CORE_WHEN_VARIABLES = [
  "terminalFocus",
  "terminalOpen",
  "isWeb",
  "isDesktop",
  "true",
  "false",
]         ;

const DEFAULT_WHEN_VARIABLES = new Set        (CORE_WHEN_VARIABLES);
for (const binding of DEFAULT_RESOLVED_KEYBINDINGS) {
  collectWhenIdentifiersFromNode(binding.whenAst, DEFAULT_WHEN_VARIABLES);
}

const DEFAULT_WHEN_VARIABLE =
  [...DEFAULT_WHEN_VARIABLES].find(
    (identifier) => identifier !== "true" && identifier !== "false",
  ) ?? "terminalFocus";
const KNOWN_WHEN_VARIABLES = new Set(DEFAULT_WHEN_VARIABLES);

function shortcutToKeybindingInput(shortcut                    )         {
  const parts           = [];
  if (shortcut.modKey) parts.push("mod");
  if (shortcut.metaKey) parts.push("meta");
  if (shortcut.ctrlKey) parts.push("ctrl");
  if (shortcut.altKey) parts.push("alt");
  if (shortcut.shiftKey) parts.push("shift");
  parts.push(shortcut.key === " " ? "space" : shortcut.key === "escape" ? "esc" : shortcut.key);
  return parts.join("+");
}

function whenAstToExpression(node                                )         {
  if (!node) return "";
  switch (node.type) {
    case "identifier":
      return node.name;
    case "not":
      return `!${wrapWhenExpression(node.node)}`;
    case "and":
      return `${wrapWhenExpression(node.left)} && ${wrapWhenExpression(node.right)}`;
    case "or":
      return `${wrapWhenExpression(node.left)} || ${wrapWhenExpression(node.right)}`;
  }
}

function whenNodeRemoveLabel(node                    , depth        )         {
  if (depth === 0) return "Clear all conditions";
  if (node.type === "identifier" || (node.type === "not" && node.node.type === "identifier")) {
    return "Remove condition";
  }
  return "Remove group and its conditions";
}

function wrapWhenExpression(node                    )         {
  if (node.type === "identifier" || node.type === "not") return whenAstToExpression(node);
  return `(${whenAstToExpression(node)})`;
}

function parseWhenExpressionDraft(
  expression        ,
)                                                                                       {
  const trimmed = expression.trim();
  if (trimmed.length === 0) return { ok: true, value: undefined };

  const ast = parseKeybindingWhenExpression(trimmed);
  if (!ast) {
    return {
      ok: false,
      message: "Use variables with !, &&, ||, and parentheses.",
    };
  }

  return { ok: true, value: ast };
}

function sourceForBinding(binding                        )                   {
  if (String(binding.command).startsWith("script.")) {
    return "Project";
  }

  const bindingKey = shortcutToKeybindingInput(binding.shortcut);
  const bindingWhen = whenAstToExpression(binding.whenAst);
  const isDefault = DEFAULT_RESOLVED_KEYBINDINGS.some(
    (entry) =>
      entry.command === binding.command &&
      shortcutToKeybindingInput(entry.shortcut) === bindingKey &&
      whenAstToExpression(entry.whenAst) === bindingWhen,
  );

  return isDefault ? "Default" : "Custom";
}

function defaultBindingForBinding(
  binding                        ,
)                                     {
  const bindingKey = shortcutToKeybindingInput(binding.shortcut);
  const bindingWhen = whenAstToExpression(binding.whenAst);

  return (
    DEFAULT_RESOLVED_KEYBINDINGS.find(
      (entry) =>
        entry.command === binding.command &&
        shortcutToKeybindingInput(entry.shortcut) === bindingKey &&
        whenAstToExpression(entry.whenAst) === bindingWhen,
    ) ??
    DEFAULT_RESOLVED_KEYBINDINGS.find(
      (entry) =>
        entry.command === binding.command && whenAstToExpression(entry.whenAst) === bindingWhen,
    ) ??
    DEFAULT_RESOLVED_KEYBINDINGS.find((entry) => entry.command === binding.command)
  );
}

function keybindingRowId(command                   , key        , when        )         {
  return `${command}\u0000${key}\u0000${when}`;
}

function conflictsWithWhen(leftWhen        , rightWhen        )          {
  return leftWhen.length === 0 || rightWhen.length === 0 || leftWhen === rightWhen;
}

function keybindingConflictLabels(
  rows                              ,
  input                                                                         ,
)                        {
  if (input.key.trim().length === 0) return [];
  const conflicts                = [];
  for (const candidate of rows) {
    if (
      candidate.id !== input.rowId &&
      candidate.key === input.key &&
      conflictsWithWhen(candidate.when, input.when)
    ) {
      conflicts.push(commandLabel(candidate.command));
    }
  }
  return [...new Set(conflicts)].slice().sort();
}

                                  
                      
                         
                                              
 

const KEYBINDING_GROUPS = [
  {
    id: "navigation",
    title: "Navigation",
    prefixes: ["sidebar", "rightPanel", "commandPalette", "filePicker", "projectSearch", "editor"],
  },
  { id: "threads", title: "Threads", prefixes: ["thread", "chat", "pullRequest"] },
  { id: "composer", title: "Composer", prefixes: ["composer", "modelPicker"] },
  { id: "terminal", title: "Terminal", prefixes: ["terminal"] },
  { id: "preview", title: "Preview & diff", prefixes: ["preview", "diff"] },
  { id: "appearance", title: "Appearance", prefixes: ["theme", "appearance", "themeEditor"] },
  { id: "scripts", title: "Project scripts", prefixes: ["script"] },
]         ;
const OTHER_KEYBINDING_GROUP = { id: "other", title: "Other" }         ;

function keybindingGroupFor(command                   )                                {
  const prefix = String(command).split(".")[0] ?? "";
  return (
    KEYBINDING_GROUPS.find((group) => (group.prefixes                         ).includes(prefix)) ??
    OTHER_KEYBINDING_GROUP
  );
}

function groupKeybindingRows(
  rows                              ,
)                                 {
  const rowsByGroup = new Map                              ();
  for (const row of rows) {
    const group = keybindingGroupFor(row.command);
    const bucket = rowsByGroup.get(group.id);
    if (bucket) bucket.push(row);
    else rowsByGroup.set(group.id, [row]);
  }
  return flattenNativeGroups([...KEYBINDING_GROUPS, OTHER_KEYBINDING_GROUP], (group) => {
    const groupRows = rowsByGroup.get(group.id);
    return groupRows ? [{ id: group.id, title: group.title, rows: groupRows }] : [];
  });
}

function buildKeybindingRows(
  keybindings                           ,
)                               {
  const rowsById = new Map                       ();
  for (const binding of keybindings) {
    const defaultBinding = defaultBindingForBinding(binding);
    const key = shortcutToKeybindingInput(binding.shortcut);
    const when = whenAstToExpression(binding.whenAst);
    const id = keybindingRowId(binding.command, key, when);
    rowsById.set(id, {
      id,
      command: binding.command,
      key,
      when,
      source: sourceForBinding(binding),
      defaultKey: defaultBinding ? shortcutToKeybindingInput(defaultBinding.shortcut) : null,
      defaultWhen: whenAstToExpression(defaultBinding?.whenAst),
      binding,
      conflicts: [],
    });
  }
  const rows = [...rowsById.values()];

  const rowsWithConflicts = rows.map((row) => {
    const conflicts = keybindingConflictLabels(rows, {
      rowId: row.id,
      key: row.key,
      when: row.when,
    });
    return conflicts.length > 0
      ? Object.assign({}, row, { conflicts: [...new Set(conflicts)].slice().sort() })
      : row;
  });

  rowsWithConflicts.sort((left, right) => {
    const commandCompare = compareCommands(left.command, right.command, (command) => command);
    if (commandCompare !== 0) return commandCompare;
    return left.key.localeCompare(right.key);
  });

  return rowsWithConflicts;
}

// Filtering keeps the row objects so rows that stay visible while the query
// changes receive the same props and skip rendering.
function filterKeybindingRows(
  rows                              ,
  query        ,
)                               {
  const normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery.length === 0) return rows;
  return rows.filter((row) => {
    return (
      row.command.toLowerCase().includes(normalizedQuery) ||
      commandLabel(row.command).toLowerCase().includes(normalizedQuery) ||
      row.key.toLowerCase().includes(normalizedQuery) ||
      row.when.toLowerCase().includes(normalizedQuery) ||
      row.source.toLowerCase().includes(normalizedQuery)
    );
  });
}

function collectWhenIdentifiersFromNode(
  node                                ,
  identifiers             ,
)       {
  if (!node) return;
  switch (node.type) {
    case "identifier":
      identifiers.add(node.name);
      return;
    case "not":
      collectWhenIdentifiersFromNode(node.node, identifiers);
      return;
    case "and":
    case "or":
      collectWhenIdentifiersFromNode(node.left, identifiers);
      collectWhenIdentifiersFromNode(node.right, identifiers);
      return;
  }
}

function isKnownWhenVariable(identifier        )          {
  return KNOWN_WHEN_VARIABLES.has(identifier);
}

function unknownWhenVariables(node                                )                        {
  const identifiers = new Set        ();
  collectWhenIdentifiersFromNode(node, identifiers);
  return [...identifiers].filter((identifier) => !isKnownWhenVariable(identifier)).slice().sort();
}

function buildWhenVariableOptions()                                    {
  return [...KNOWN_WHEN_VARIABLES].slice().sort((left, right) => {
    const leftCoreIndex = CORE_WHEN_VARIABLES.indexOf(left                                        );
    const rightCoreIndex = CORE_WHEN_VARIABLES.indexOf(
      right                                        ,
    );
    if (leftCoreIndex !== -1 || rightCoreIndex !== -1) {
      return (
        (leftCoreIndex === -1 ? Number.MAX_SAFE_INTEGER : leftCoreIndex) -
        (rightCoreIndex === -1 ? Number.MAX_SAFE_INTEGER : rightCoreIndex)
      );
    }
    return left.localeCompare(right);
  });
}

function buildKeybindingCommandOptions(
  keybindings                           ,
)                                         {
  const commands = new Set                   (STATIC_KEYBINDING_COMMANDS);
  for (const binding of keybindings) {
    commands.add(binding.command);
  }
  return [...commands].slice().sort((left, right) => compareCommands(left, right, commandLabel));
}

function commandLabel(command                   )         {
  if (command === "composer.sendAlternate") return "Composer: Opposite Queue or Steer Action";
  if (command === "composer.sendBackground") return "Composer: Start in Background";
  if (command === "composer.sendAndNewThread") return "Composer: Send and Start New Thread";
  if (command === "thread.steerQueuedMessage") return "Queue: Send First Queued Message as Steer";
  if (command === "thread.editQueuedMessage") return "Queue: Edit Last Queued Message";
  if (command === "thread.copyReference") return "Pull Request: Copy Link or Thread ID";
  const usageMetric = METRIC_OPTIONS.find((option) => option.command === command);
  if (usageMetric) return `Usage: ${usageMetric.label}`;
  const usagePeriod = WINDOW_OPTIONS.find((option) => option.command === command);
  if (usagePeriod) return `Usage: Period: ${usagePeriod.label}`;
  if (command === "view.reopenClosed") return "Reopen Closed Tab";
  if (command === "preview.focusUrl") return "Preview: Focus URL";
  const raw = String(command);
  if (raw.startsWith("script.") && raw.endsWith(".run")) {
    return `Run Script: ${titleCaseCommandSegment(raw.slice("script.".length, -".run".length))}`;
  }
  return raw.split(".").map(titleCaseCommandSegment).join(": ");
}

function titleCaseCommandSegment(segment        )         {
  const words                = [];
  for (const part of segment.replace(/([a-z0-9])([A-Z])/g, "$1 $2").split(/[-_\s]+/)) {
    if (part.length > 0) {
      words.push(part.slice(0, 1).toUpperCase() + part.slice(1));
    }
  }
  return words.join(" ");
}

function recordedLayoutShortcutKey(
  event                                                             ,
)         {
  const physicalKey = shortcutKeyFromEvent(event);
  const layoutKey = event.key.toLowerCase();
  if (event.altKey || layoutKey.length !== 1) return physicalKey;
  // Normalize only US shifted glyphs at their US position. Other layouts can
  // produce the same glyph without Shift or at a different position.
  const unshiftedKey = event.shiftKey ? US_SHIFTED_TO_UNSHIFTED_KEYS.get(layoutKey) : undefined;
  return unshiftedKey === physicalKey ? physicalKey : layoutKey;
}

function normalizeShortcutKeyToken(key        )                {
  const normalized = key.toLowerCase();
  if (
    normalized === "meta" ||
    normalized === "control" ||
    normalized === "ctrl" ||
    normalized === "shift" ||
    normalized === "alt" ||
    normalized === "option"
  ) {
    return null;
  }
  if (normalized === " ") return "space";
  if (normalized === "escape") return "esc";
  if (normalized === "arrowup") return "arrowup";
  if (normalized === "arrowdown") return "arrowdown";
  if (normalized === "arrowleft") return "arrowleft";
  if (normalized === "arrowright") return "arrowright";
  if (normalized.length === 1) return normalized;
  if (/^f\d{1,2}$/.test(normalized)) return normalized;
  if (normalized === "enter" || normalized === "tab" || normalized === "backspace") {
    return normalized;
  }
  if (normalized === "delete" || normalized === "home" || normalized === "end") {
    return normalized;
  }
  if (normalized === "pageup" || normalized === "pagedown") return normalized;
  return null;
}

function keybindingFromKeyboardEvent(
  event                                                                                     ,
  platform        ,
  options    
                                       
   ,
)                {
  const keyToken = normalizeShortcutKeyToken(
    options?.layoutCharacter ? recordedLayoutShortcutKey(event) : shortcutKeyFromEvent(event),
  );
  if (!keyToken) return null;

  const parts           = [];
  if (isMacPlatform(platform)) {
    if (event.metaKey) parts.push("mod");
    if (event.ctrlKey) parts.push("ctrl");
  } else {
    if (event.ctrlKey) parts.push("mod");
    if (event.metaKey) parts.push("meta");
  }
  if (event.altKey) parts.push("alt");
  if (event.shiftKey) parts.push("shift");
  parts.push(keyToken);
  return parts.join("+");
}

function flattenWhenChildren(
  node                    ,
  operator                 ,
)                       {
  if (node.type !== operator) return [node];
  return [
    ...flattenWhenChildren(node.left, operator),
    ...flattenWhenChildren(node.right, operator),
  ];
}

function buildWhenExpressionGroup(
  children                               ,
  operator                 ,
)                                 {
  const first = children[0];
  if (!first) return undefined;
  return children.slice(1).reduce                    (
    (left, right) => ({
      type: operator,
      left,
      right,
    }),
    first,
  );
}

function conditionParts(node                    )                                                  {
  if (node.type === "identifier") return { identifier: node.name, negated: false };
  if (node.type === "not" && node.node.type === "identifier") {
    return { identifier: node.node.name, negated: true };
  }
  return null;
}

function setConditionIdentifier(node                    , identifier        )                     {
  const parts = conditionParts(node);
  if (!parts) return node;
  const next                     = { type: "identifier", name: identifier };
  return parts.negated ? { type: "not", node: next } : next;
}

function setConditionNegated(node                    , negated         )                     {
  const parts = conditionParts(node);
  if (!parts) return negated ? { type: "not", node } : node;
  const identifier                     = { type: "identifier", name: parts.identifier };
  return negated ? { type: "not", node: identifier } : identifier;
}

function defaultWhenCondition()                     {
  return { type: "identifier", name: DEFAULT_WHEN_VARIABLE };
}

function defaultWhenGroup(operator                  = "and")                     {
  return {
    type: operator,
    left: defaultWhenCondition(),
    right: { type: "not", node: defaultWhenCondition() },
  };
}


function formatShortcutKeyLabel(key        )         {
  if (key === " ") return "Space";
  if (key.length === 1) return key.toUpperCase();
  if (key === "escape") return "Esc";
  if (key === "arrowup") return "Up";
  if (key === "arrowdown") return "Down";
  if (key === "arrowleft") return "Left";
  if (key === "arrowright") return "Right";
  return key.slice(0, 1).toUpperCase() + key.slice(1);
}

function formatShortcutLabel(
  shortcut                    ,
  platform = navigator.platform,
)         {
  const keyLabel = formatShortcutKeyLabel(shortcut.key);
  const useMetaForMod = isMacPlatform(platform);
  const showMeta = shortcut.metaKey || (shortcut.modKey && useMetaForMod);
  const showCtrl = shortcut.ctrlKey || (shortcut.modKey && !useMetaForMod);
  const showAlt = shortcut.altKey;
  const showShift = shortcut.shiftKey;

  if (useMetaForMod) {
    return `${showCtrl ? "\u2303" : ""}${showAlt ? "\u2325" : ""}${showShift ? "\u21e7" : ""}${showMeta ? "\u2318" : ""}${keyLabel}`;
  }

  const parts           = [];
  if (showCtrl) parts.push("Ctrl");
  if (showAlt) parts.push("Alt");
  if (showShift) parts.push("Shift");
  if (showMeta) parts.push("Meta");
  parts.push(keyLabel);
  return parts.join("+");
}


const EDITORS = [
  {
    id: "cursor",
    label: "Cursor",
    commands: ["cursor"],
    // File and workspace opens must target the IDE even when the Agents Window is active.
    baseArgs: ["--classic"],
    launchStyle: "goto",
    remoteScheme: "cursor",
  },
  { id: "trae", label: "Trae", commands: ["trae"], launchStyle: "goto" },
  { id: "kiro", label: "Kiro", commands: ["kiro"], baseArgs: ["ide"], launchStyle: "goto" },
  {
    id: "vscode",
    label: "VS Code",
    commands: ["code"],
    launchStyle: "goto",
    remoteScheme: "vscode",
  },
  {
    id: "vscode-insiders",
    label: "VS Code Insiders",
    commands: ["code-insiders"],
    launchStyle: "goto",
    remoteScheme: "vscode-insiders",
  },
  {
    id: "vscodium",
    label: "VSCodium",
    commands: ["codium"],
    launchStyle: "goto",
    remoteScheme: "vscodium",
  },
  {
    id: "zed",
    label: "Zed",
    commands: ["zed", "zeditor"],
    launchStyle: "direct-path",
    remoteScheme: "zed",
  },
  {
    id: "antigravity",
    label: "Antigravity",
    // `agy` is the standalone Antigravity CLI, not the IDE. The IDE bundle
    // ships `antigravity-ide`, so it comes first for install-folder lookups.
    commands: ["antigravity-ide", "agy-ide"],
    launchStyle: "goto",
  },
  {
    id: "idea",
    label: "IntelliJ IDEA",
    commands: ["idea"],
    launchStyle: "line-column",
    jetbrainsProductCode: "IU",
  },
  {
    id: "aqua",
    label: "Aqua",
    commands: ["aqua"],
    launchStyle: "line-column",
    jetbrainsProductCode: "QA",
  },
  {
    id: "clion",
    label: "CLion",
    commands: ["clion"],
    launchStyle: "line-column",
    jetbrainsProductCode: "CL",
  },
  {
    id: "datagrip",
    label: "DataGrip",
    commands: ["datagrip"],
    launchStyle: "line-column",
    jetbrainsProductCode: "DB",
  },
  {
    id: "dataspell",
    label: "DataSpell",
    commands: ["dataspell"],
    launchStyle: "line-column",
    jetbrainsProductCode: "DS",
  },
  {
    id: "goland",
    label: "GoLand",
    commands: ["goland"],
    launchStyle: "line-column",
    jetbrainsProductCode: "GO",
  },
  {
    id: "phpstorm",
    label: "PhpStorm",
    commands: ["phpstorm"],
    launchStyle: "line-column",
    jetbrainsProductCode: "PS",
  },
  {
    id: "pycharm",
    label: "PyCharm",
    commands: ["pycharm"],
    launchStyle: "line-column",
    jetbrainsProductCode: "PY",
  },
  {
    id: "rider",
    label: "Rider",
    commands: ["rider"],
    launchStyle: "line-column",
    jetbrainsProductCode: "RD",
  },
  {
    id: "rubymine",
    label: "RubyMine",
    commands: ["rubymine"],
    launchStyle: "line-column",
    jetbrainsProductCode: "RM",
  },
  {
    id: "rustrover",
    label: "RustRover",
    commands: ["rustrover"],
    launchStyle: "line-column",
    jetbrainsProductCode: "RR",
  },
  {
    id: "webstorm",
    label: "WebStorm",
    commands: ["webstorm"],
    launchStyle: "line-column",
    jetbrainsProductCode: "WS",
  },
  { id: "file-manager", label: "File Manager", commands: null, launchStyle: "direct-path" },
]                                                   ;


function resolvePlatform(options                                  )         {
  return options?.platform ?? navigator.platform;
}

function resolveContext(options                                  )                       {
  return Object.assign({
    terminalFocus: false,
    terminalOpen: false,
    previewFocus: false,
    previewOpen: false,
    isWeb: !isElectron,
    isDesktop: isElectron,
    editableFocus: false,
  }, options?.context);
}

function evaluateWhenNode(node                    , context                      )          {
  switch (node.type) {
    case "identifier":
      if (node.name === "true") return true;
      if (node.name === "false") return false;
      return Boolean(context[node.name]);
    case "not":
      return !evaluateWhenNode(node.node, context);
    case "and":
      return evaluateWhenNode(node.left, context) && evaluateWhenNode(node.right, context);
    case "or":
      return evaluateWhenNode(node.left, context) || evaluateWhenNode(node.right, context);
  }
}

function matchesWhenClause(
  whenAst                                ,
  context                      ,
)          {
  if (!whenAst) return true;
  return evaluateWhenNode(whenAst, context);
}

function shortcutConflictKey(
  shortcut                    ,
  platform = navigator.platform,
)         {
  const useMetaForMod = isMacPlatform(platform);
  const metaKey = shortcut.metaKey || (shortcut.modKey && useMetaForMod);
  const ctrlKey = shortcut.ctrlKey || (shortcut.modKey && !useMetaForMod);
  return [
    shortcut.key,
    metaKey ? "meta" : "",
    ctrlKey ? "ctrl" : "",
    shortcut.shiftKey ? "shift" : "",
    shortcut.altKey ? "alt" : "",
  ].join("|");
}

function shortcutForContext(binding                        , context                      ) {
  const shortcut = binding.shortcut;
  // Existing installs persist Mod+Enter. Resolve its alternate chord against
  // the send preference without rewriting saved or remapped shortcuts.
  if (
    context.composerRequiresModifier &&
    binding.command === "composer.sendAlternate" &&
    shortcut.key === "enter" &&
    shortcut.modKey &&
    !shortcut.metaKey &&
    !shortcut.ctrlKey &&
    !shortcut.altKey &&
    !shortcut.shiftKey
  ) {
    return Object.assign({}, shortcut, { shiftKey: true });
  }
  return shortcut;
}

function effectiveShortcutsForCommand(
  keybindings                           ,
  command                   ,
  options                       ,
)                       {
  const platform = resolvePlatform(options);
  const context = resolveContext(options);
  const claimedShortcuts = new Set        ();
  const effective                       = [];

  for (let index = keybindings.length - 1; index >= 0; index -= 1) {
    const binding = keybindings[index];
    if (!binding) continue;
    if (!matchesWhenClause(binding.whenAst, context)) continue;

    const shortcut = shortcutForContext(binding, context);
    const conflictKey = shortcutConflictKey(shortcut, platform);
    if (claimedShortcuts.has(conflictKey)) {
      continue;
    }

    claimedShortcuts.add(conflictKey);
    if (binding.command === command) {
      effective.push(shortcut);
    }
  }

  return effective;
}

function findEffectiveShortcutForCommand(
  keybindings                           ,
  command                   ,
  options                       ,
)                            {
  return effectiveShortcutsForCommand(keybindings, command, options)[0] ?? null;
}

function matchesCommandShortcut(
  event                   ,
  keybindings                           ,
  command                   ,
  options                       ,
)          {
  return resolveShortcutCommand(event, keybindings, options) === command;
}

function resolveShortcutCommand(
  event                   ,
  keybindings                           ,
  options                       ,
)                           {
  const platform = resolvePlatform(options);
  const context = resolveContext(options);

  for (let index = keybindings.length - 1; index >= 0; index -= 1) {
    const binding = keybindings[index];
    if (!binding) continue;
    if (!matchesWhenClause(binding.whenAst, context)) continue;
    if (!matchesKeybindingShortcut(event, shortcutForContext(binding, context), platform)) continue;
    return binding.command;
  }
  return null;
}

function resolveChatShortcutCommand(
  event                   ,
  primaryKeybindings                           ,
  activeKeybindings                           ,
  options                       ,
)                           {
  const primaryCommand = resolveShortcutCommand(event, primaryKeybindings, options);
  if (primaryCommand !== null && projectScriptIdFromCommand(primaryCommand) === null) {
    return primaryCommand;
  }

  const activeCommand = resolveShortcutCommand(event, activeKeybindings, options);
  return activeCommand !== null && projectScriptIdFromCommand(activeCommand) !== null
    ? activeCommand
    : null;
}


function projectScriptIdFromCommand(command        )                {
  const trimmed = command.trim();
  if (!isScriptRunCommand(trimmed)) {
    return null;
  }
  const [prefix, , suffix] = SCRIPT_RUN_COMMAND_PATTERN.parts;
  return trimmed.slice(prefix.literal.length, -suffix.literal.length);
}


return {EVENT_CODE_SHORTCUT_KEYS,US_SHIFTED_TO_UNSHIFTED_KEYS,isMacPlatform,normalizeEventKey,shortcutKeyFromEvent,resolveEventKeys,matchesKeybindingShortcutModifiers,matchesKeybindingShortcut,DEFAULT_KEYBINDINGS,parseKeybindingShortcut,parseKeybindingWhenExpression,compileResolvedKeybindingRule,compileResolvedKeybindingsConfig,DEFAULT_RESOLVED_KEYBINDINGS,mergeWithDefaultKeybindings,DEFAULT_WHEN_VARIABLE,shortcutToKeybindingInput,whenAstToExpression,whenNodeRemoveLabel,parseWhenExpressionDraft,keybindingConflictLabels,groupKeybindingRows,buildKeybindingRows,filterKeybindingRows,isKnownWhenVariable,unknownWhenVariables,buildWhenVariableOptions,buildKeybindingCommandOptions,commandLabel,keybindingFromKeyboardEvent,STATIC_KEYBINDING_COMMANDS,flattenWhenChildren,buildWhenExpressionGroup,conditionParts,setConditionIdentifier,setConditionNegated,defaultWhenCondition,defaultWhenGroup,formatShortcutKeyLabel,formatShortcutLabel,EDITORS,evaluateWhenNode,matchesWhenClause,shortcutConflictKey,effectiveShortcutsForCommand,resolveShortcutCommand,resolveChatShortcutCommand,projectScriptIdFromCommand};
})();
