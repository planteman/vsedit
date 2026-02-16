// vsedit Extension Host — runs VS Code extensions unchanged
// Communicates with Rust main process via Content-Length framed messages
// Wire format: { type: "request"|"response"|"event", ... }
'use strict';

const path = require('path');
const fs = require('fs');

// ─── RPC Transport (Content-Length framing, vsedit wire format) ─────────────────

class RpcTransport {
    constructor() {
        this._pendingRequests = new Map();
        this._nextId = 1;
        this._handlers = new Map();
        this._buffer = Buffer.alloc(0);
        this._contentLength = -1;
    }

    start() {
        process.stdin.on('data', (chunk) => this._onData(chunk));
    }

    _onData(chunk) {
        this._buffer = Buffer.concat([this._buffer, chunk]);
        while (true) {
            if (this._contentLength === -1) {
                const headerEnd = this._buffer.indexOf('\r\n\r\n');
                if (headerEnd === -1) return;
                const header = this._buffer.slice(0, headerEnd).toString('utf8');
                const match = header.match(/Content-Length:\s*(\d+)/i);
                if (!match) { this._buffer = Buffer.alloc(0); return; }
                this._contentLength = parseInt(match[1], 10);
                this._buffer = this._buffer.slice(headerEnd + 4);
            }
            if (this._buffer.length < this._contentLength) return;
            const body = this._buffer.slice(0, this._contentLength).toString('utf8');
            this._buffer = this._buffer.slice(this._contentLength);
            this._contentLength = -1;
            try { this._handleMessage(JSON.parse(body)); }
            catch (e) { /* skip malformed */ }
        }
    }

    _handleMessage(msg) {
        switch (msg.type) {
            case 'request': {
                // Incoming request from host: { type, id, proxyId, method, args }
                const key = msg.proxyId ? `${msg.proxyId}/${msg.method}` : msg.method;
                const handler = this._handlers.get(key) || this._handlers.get(msg.method);
                if (handler) {
                    // Extract params: if args has one object element, unwrap it
                    const params = (msg.args && msg.args.length === 1 && typeof msg.args[0] === 'object' && msg.args[0] !== null)
                        ? msg.args[0]
                        : (msg.args || {});
                    Promise.resolve(handler(params)).then(result => {
                        this._send({ type: 'response', id: msg.id, result: result ?? null });
                    }).catch(err => {
                        this._send({ type: 'response', id: msg.id, error: { message: err.message } });
                    });
                } else {
                    this._send({ type: 'response', id: msg.id, error: { message: `Unknown method: ${key}` } });
                }
                break;
            }
            case 'response': {
                // Response to our outgoing request
                const pending = this._pendingRequests.get(msg.id);
                if (pending) {
                    this._pendingRequests.delete(msg.id);
                    if (msg.error) pending.reject(new Error(msg.error.message));
                    else pending.resolve(msg.result);
                }
                break;
            }
            case 'event': {
                // Incoming event/notification: { type, proxyId, eventName, data }
                const key = msg.proxyId ? `${msg.proxyId}/${msg.eventName}` : msg.eventName;
                const handler = this._handlers.get(key) || this._handlers.get(msg.eventName);
                if (handler) handler(msg.data);
                break;
            }
            default: {
                // Fallback: try to handle messages that may not have 'type' field
                // (e.g., from test tooling)
                if (msg.id !== undefined && msg.method) {
                    const handler = this._handlers.get(msg.method);
                    if (handler) {
                        Promise.resolve(handler(msg.params || msg.args || {})).then(result => {
                            this._send({ type: 'response', id: msg.id, result: result ?? null });
                        }).catch(err => {
                            this._send({ type: 'response', id: msg.id, error: { message: err.message } });
                        });
                    }
                } else if (msg.id !== undefined) {
                    const pending = this._pendingRequests.get(msg.id);
                    if (pending) {
                        this._pendingRequests.delete(msg.id);
                        if (msg.error) pending.reject(new Error(msg.error.message));
                        else pending.resolve(msg.result);
                    }
                }
            }
        }
    }

    _send(msg) {
        const body = JSON.stringify(msg);
        const header = `Content-Length: ${Buffer.byteLength(body, 'utf8')}\r\n\r\n`;
        process.stdout.write(header + body);
    }

    request(method, params) {
        const id = this._nextId++;
        // Split 'proxyId/method' convention (e.g. 'mainThread/executeCommand')
        let proxyId = 'ExtHost';
        let rpcMethod = method;
        const slashIdx = method.indexOf('/');
        if (slashIdx > 0) {
            proxyId = method.substring(0, slashIdx);
            rpcMethod = method.substring(slashIdx + 1);
        }
        // Convert params object to args array for the wire format
        const args = params !== undefined ? [params] : [];
        return new Promise((resolve, reject) => {
            this._pendingRequests.set(id, { resolve, reject });
            this._send({ type: 'request', id, proxyId, method: rpcMethod, args });
        });
    }

    notify(method, params) {
        // Notifications are sent as events in the wire format
        let proxyId = 'ExtHost';
        let eventName = method;
        const slashIdx = method.indexOf('/');
        if (slashIdx > 0) {
            proxyId = method.substring(0, slashIdx);
            eventName = method.substring(slashIdx + 1);
        }
        this._send({ type: 'event', proxyId, eventName, data: params ?? {} });
    }

    onRequest(method, handler) { this._handlers.set(method, handler); }
    onNotification(method, handler) { this._handlers.set(method, handler); }
}

const rpc = new RpcTransport();

// ─── VS Code Types ─────────────────────────────────────────────────────────────

class Position {
    constructor(line, character) { this.line = line; this.character = character; }
    translate(lineDelta = 0, charDelta = 0) { return new Position(this.line + lineDelta, this.character + charDelta); }
    with(line, character) { return new Position(line ?? this.line, character ?? this.character); }
    isEqual(other) { return this.line === other.line && this.character === other.character; }
    isBefore(other) { return this.line < other.line || (this.line === other.line && this.character < other.character); }
    isAfter(other) { return !this.isEqual(other) && !this.isBefore(other); }
    compareTo(other) { return this.line !== other.line ? this.line - other.line : this.character - other.character; }
}

class Range {
    constructor(startOrStartLine, startCharOrEnd, endLine, endChar) {
        if (startOrStartLine instanceof Position) {
            this.start = startOrStartLine;
            this.end = startCharOrEnd instanceof Position ? startCharOrEnd : startOrStartLine;
        } else {
            this.start = new Position(startOrStartLine, startCharOrEnd);
            this.end = new Position(endLine, endChar);
        }
    }
    get isEmpty() { return this.start.isEqual(this.end); }
    get isSingleLine() { return this.start.line === this.end.line; }
    contains(posOrRange) {
        if (posOrRange instanceof Range) return this.contains(posOrRange.start) && this.contains(posOrRange.end);
        return !posOrRange.isBefore(this.start) && !posOrRange.isAfter(this.end);
    }
    intersection(other) {
        const start = this.start.isAfter(other.start) ? this.start : other.start;
        const end = this.end.isBefore(other.end) ? this.end : other.end;
        if (start.isAfter(end)) return undefined;
        return new Range(start, end);
    }
    union(other) {
        const start = this.start.isBefore(other.start) ? this.start : other.start;
        const end = this.end.isAfter(other.end) ? this.end : other.end;
        return new Range(start, end);
    }
    with(start, end) { return new Range(start ?? this.start, end ?? this.end); }
    isEqual(other) { return this.start.isEqual(other.start) && this.end.isEqual(other.end); }
}

class Selection extends Range {
    constructor(anchorOrAnchorLine, activeOrAnchorChar, activeLine, activeChar) {
        if (anchorOrAnchorLine instanceof Position) {
            super(anchorOrAnchorLine, activeOrAnchorChar);
            this.anchor = anchorOrAnchorLine;
            this.active = activeOrAnchorChar;
        } else {
            const anchor = new Position(anchorOrAnchorLine, activeOrAnchorChar);
            const active = new Position(activeLine, activeChar);
            super(anchor.isBefore(active) ? anchor : active, anchor.isBefore(active) ? active : anchor);
            this.anchor = anchor;
            this.active = active;
        }
    }
    get isReversed() { return this.anchor.isAfter(this.active); }
}

class Uri {
    constructor(scheme, authority, path, query, fragment) {
        this.scheme = scheme || 'file';
        this.authority = authority || '';
        this.path = path || '';
        this.query = query || '';
        this.fragment = fragment || '';
    }
    static file(p) { return new Uri('file', '', path.resolve(p)); }
    static parse(value) {
        const m = value.match(/^([a-z][a-z0-9+.-]*):\/\/(([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?$/i);
        if (!m) return new Uri('file', '', value);
        return new Uri(m[1], m[3] || '', m[4] || '', m[6] || '', m[8] || '');
    }
    static from(components) { return new Uri(components.scheme, components.authority, components.path, components.query, components.fragment); }
    get fsPath() { return this.path; }
    toString() {
        let result = `${this.scheme}://`;
        if (this.authority) result += this.authority;
        result += this.path;
        if (this.query) result += `?${this.query}`;
        if (this.fragment) result += `#${this.fragment}`;
        return result;
    }
    with(change) { return new Uri(change.scheme ?? this.scheme, change.authority ?? this.authority, change.path ?? this.path, change.query ?? this.query, change.fragment ?? this.fragment); }
    toJSON() { return { scheme: this.scheme, authority: this.authority, path: this.path, query: this.query, fragment: this.fragment }; }
}

class Disposable {
    constructor(callOnDispose) { this._callOnDispose = callOnDispose; this._isDisposed = false; }
    static from(...disposables) { return new Disposable(() => disposables.forEach(d => d && d.dispose())); }
    dispose() { if (!this._isDisposed) { this._isDisposed = true; if (this._callOnDispose) this._callOnDispose(); } }
}

class EventEmitter {
    constructor() { this._listeners = []; }
    get event() {
        return (listener, thisArgs, disposables) => {
            const bound = thisArgs ? listener.bind(thisArgs) : listener;
            this._listeners.push(bound);
            const disposable = new Disposable(() => {
                const idx = this._listeners.indexOf(bound);
                if (idx >= 0) this._listeners.splice(idx, 1);
            });
            if (disposables) disposables.push(disposable);
            return disposable;
        };
    }
    fire(data) { this._listeners.slice().forEach(l => { try { l(data); } catch(e) { /* swallow */ } }); }
    dispose() { this._listeners = []; }
}

class CancellationTokenSource {
    constructor() {
        this._emitter = new EventEmitter();
        this.token = { isCancellationRequested: false, onCancellationRequested: this._emitter.event };
    }
    cancel() { this.token.isCancellationRequested = true; this._emitter.fire(); }
    dispose() { this._emitter.dispose(); }
}

class TextEdit {
    constructor(range, newText) { this.range = range; this.newText = newText; }
    static replace(range, newText) { return new TextEdit(range, newText); }
    static insert(position, newText) { return new TextEdit(new Range(position, position), newText); }
    static delete(range) { return new TextEdit(range, ''); }
    static setEndOfLine(eol) { const e = new TextEdit(new Range(0, 0, 0, 0), ''); e.newEol = eol; return e; }
}

class WorkspaceEdit {
    constructor() { this._edits = new Map(); }
    replace(uri, range, newText) { this._getEdits(uri).push(TextEdit.replace(range, newText)); }
    insert(uri, position, newText) { this._getEdits(uri).push(TextEdit.insert(position, newText)); }
    delete(uri, range) { this._getEdits(uri).push(TextEdit.delete(range)); }
    has(uri) { return this._edits.has(uri.toString()); }
    set(uri, edits) { this._edits.set(uri.toString(), { uri, edits }); }
    get size() { return this._edits.size; }
    entries() { return Array.from(this._edits.values()).map(e => [e.uri, e.edits]); }
    _getEdits(uri) { const k = uri.toString(); if (!this._edits.has(k)) this._edits.set(k, { uri, edits: [] }); return this._edits.get(k).edits; }
}

const DiagnosticSeverity = { Error: 0, Warning: 1, Information: 2, Hint: 3 };

class Diagnostic {
    constructor(range, message, severity) {
        this.range = range;
        this.message = message;
        this.severity = severity ?? DiagnosticSeverity.Error;
        this.source = '';
        this.code = undefined;
        this.relatedInformation = [];
        this.tags = [];
    }
}

const CompletionItemKind = {
    Text: 0, Method: 1, Function: 2, Constructor: 3, Field: 4, Variable: 5,
    Class: 6, Interface: 7, Module: 8, Property: 9, Unit: 10, Value: 11,
    Enum: 12, Keyword: 13, Snippet: 14, Color: 15, File: 16, Reference: 17,
    Folder: 18, EnumMember: 19, Constant: 20, Struct: 21, Event: 22, Operator: 23, TypeParameter: 24
};

class CompletionItem {
    constructor(label, kind) { this.label = label; this.kind = kind; this.detail = ''; this.documentation = ''; this.sortText = ''; this.filterText = ''; this.insertText = ''; this.preselect = false; }
}

class CompletionList {
    constructor(items, isIncomplete) { this.items = items || []; this.isIncomplete = isIncomplete || false; }
}

const SymbolKind = {
    File: 0, Module: 1, Namespace: 2, Package: 3, Class: 4, Method: 5,
    Property: 6, Field: 7, Constructor: 8, Enum: 9, Interface: 10, Function: 11,
    Variable: 12, Constant: 13, String: 14, Number: 15, Boolean: 16, Array: 17,
    Object: 18, Key: 19, Null: 20, EnumMember: 21, Struct: 22, Event: 23, Operator: 24, TypeParameter: 25
};

class SymbolInformation {
    constructor(name, kind, containerName, location) { this.name = name; this.kind = kind; this.containerName = containerName || ''; this.location = location; }
}

class DocumentSymbol {
    constructor(name, detail, kind, range, selectionRange) { this.name = name; this.detail = detail; this.kind = kind; this.range = range; this.selectionRange = selectionRange; this.children = []; }
}

class Location {
    constructor(uri, rangeOrPosition) {
        this.uri = uri;
        this.range = rangeOrPosition instanceof Position ? new Range(rangeOrPosition, rangeOrPosition) : rangeOrPosition;
    }
}

class Hover {
    constructor(contents, range) { this.contents = Array.isArray(contents) ? contents : [contents]; this.range = range; }
}

class MarkdownString {
    constructor(value, supportThemeIcons) { this.value = value || ''; this.isTrusted = false; this.supportThemeIcons = supportThemeIcons || false; this.supportHtml = false; }
    appendText(value) { this.value += value.replace(/[\\`*_{}[\]()#+\-.!]/g, '\\$&'); return this; }
    appendMarkdown(value) { this.value += value; return this; }
    appendCodeblock(code, language) { this.value += `\n\`\`\`${language || ''}\n${code}\n\`\`\`\n`; return this; }
}

class SnippetString {
    constructor(value) { this.value = value || ''; }
    appendText(s) { this.value += s.replace(/[$}\\]/g, '\\$&'); return this; }
    appendPlaceholder(value, number) { this.value += `\${${number || 1}:${typeof value === 'function' ? '' : value}}`; return this; }
    appendTabstop(number) { this.value += `\$${number || 0}`; return this; }
    appendVariable(name, defaultValue) { this.value += `\${${name}${defaultValue ? ':' + defaultValue : ''}}`; return this; }
    appendChoice(values, number) { this.value += `\${${number || 1}|${values.join(',')}|}`; return this; }
}

const EndOfLine = { LF: 1, CRLF: 2 };
const TextEditorRevealType = { Default: 0, InCenter: 1, InCenterIfOutsideViewport: 2, AtTop: 3 };
const StatusBarAlignment = { Left: 1, Right: 2 };
const ViewColumn = { Active: -1, Beside: -2, One: 1, Two: 2, Three: 3, Four: 4, Five: 5, Six: 6, Seven: 7, Eight: 8, Nine: 9 };
const ConfigurationTarget = { Global: 1, Workspace: 2, WorkspaceFolder: 3 };
const TextDocumentSaveReason = { Manual: 1, AfterDelay: 2, FocusOut: 3 };
const FileType = { Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 };
const TreeItemCollapsibleState = { None: 0, Collapsed: 1, Expanded: 2 };
const ProgressLocation = { SourceControl: 1, Window: 10, Notification: 15 };
const IndentAction = { None: 0, Indent: 1, IndentOutdent: 2, Outdent: 3 };

class ThemeColor { constructor(id) { this.id = id; } }
class ThemeIcon { constructor(id, color) { this.id = id; this.color = color; } }
ThemeIcon.File = new ThemeIcon('file');
ThemeIcon.Folder = new ThemeIcon('folder');

class TreeItem {
    constructor(labelOrUri, collapsibleState) {
        if (typeof labelOrUri === 'string') this.label = labelOrUri;
        else this.resourceUri = labelOrUri;
        this.collapsibleState = collapsibleState ?? TreeItemCollapsibleState.None;
    }
}

class CodeAction {
    constructor(title, kind) { this.title = title; this.kind = kind; this.diagnostics = []; this.isPreferred = false; }
}

const CodeActionKind = {
    Empty: '', QuickFix: 'quickfix', Refactor: 'refactor', RefactorExtract: 'refactor.extract',
    RefactorInline: 'refactor.inline', RefactorRewrite: 'refactor.rewrite', Source: 'source',
    SourceOrganizeImports: 'source.organizeImports', SourceFixAll: 'source.fixAll'
};

class CodeLens {
    constructor(range, command) { this.range = range; this.command = command; this.isResolved = !!command; }
}

const DecorationRangeBehavior = { OpenOpen: 0, ClosedClosed: 1, OpenClosed: 2, ClosedOpen: 3 };
const OverviewRulerLane = { Left: 1, Center: 2, Right: 4, Full: 7 };

class FoldingRange {
    constructor(start, end, kind) { this.start = start; this.end = end; this.kind = kind; }
}
const FoldingRangeKind = { Comment: 1, Imports: 2, Region: 3 };

class SemanticTokensLegend {
    constructor(tokenTypes, tokenModifiers) { this.tokenTypes = tokenTypes; this.tokenModifiers = tokenModifiers || []; }
}

class SemanticTokensBuilder {
    constructor(legend) { this._legend = legend; this._data = []; this._prevLine = 0; this._prevChar = 0; }
    push(line, char, length, tokenType, tokenModifiers) {
        this._data.push(line - this._prevLine, line === this._prevLine ? char - this._prevChar : char, length, tokenType, tokenModifiers || 0);
        this._prevLine = line; this._prevChar = char;
    }
    build() { return { data: new Uint32Array(this._data) }; }
}

class SignatureHelp {
    constructor() { this.signatures = []; this.activeSignature = 0; this.activeParameter = 0; }
}

class SignatureInformation {
    constructor(label, documentation) { this.label = label; this.documentation = documentation; this.parameters = []; this.activeParameter = -1; }
}

class ParameterInformation {
    constructor(label, documentation) { this.label = label; this.documentation = documentation; }
}

// ─── Extension Registry ────────────────────────────────────────────────────────

const extensions = new Map();
const commandHandlers = new Map();
const diagnosticCollections = new Map();
const treeDataProviders = new Map();

// ─── Text Document Proxy ───────────────────────────────────────────────────────

class TextDocumentProxy {
    constructor(uri, languageId, version, content) {
        this._uri = typeof uri === 'string' ? Uri.parse(uri) : uri;
        this._languageId = languageId;
        this._version = version;
        this._content = content;
        this._lines = content.split(/\r?\n/);
    }
    get uri() { return this._uri; }
    get fileName() { return this._uri.fsPath; }
    get languageId() { return this._languageId; }
    get version() { return this._version; }
    get isDirty() { return false; }
    get isUntitled() { return this._uri.scheme === 'untitled'; }
    get lineCount() { return this._lines.length; }
    getText(range) {
        if (!range) return this._content;
        const startOff = this.offsetAt(range.start);
        const endOff = this.offsetAt(range.end);
        return this._content.substring(startOff, endOff);
    }
    lineAt(lineOrPos) {
        const lineNum = typeof lineOrPos === 'number' ? lineOrPos : lineOrPos.line;
        const text = this._lines[lineNum] || '';
        const firstNonWs = text.search(/\S/);
        return {
            lineNumber: lineNum, text, range: new Range(lineNum, 0, lineNum, text.length),
            rangeIncludingLineBreak: new Range(lineNum, 0, lineNum + 1, 0),
            firstNonWhitespaceCharacterIndex: firstNonWs === -1 ? text.length : firstNonWs,
            isEmptyOrWhitespace: text.trim().length === 0
        };
    }
    offsetAt(position) {
        let offset = 0;
        for (let i = 0; i < position.line && i < this._lines.length; i++) offset += this._lines[i].length + 1;
        return offset + Math.min(position.character, (this._lines[position.line] || '').length);
    }
    positionAt(offset) {
        let remaining = offset;
        for (let i = 0; i < this._lines.length; i++) {
            if (remaining <= this._lines[i].length) return new Position(i, remaining);
            remaining -= this._lines[i].length + 1;
        }
        return new Position(this._lines.length - 1, (this._lines[this._lines.length - 1] || '').length);
    }
    getWordRangeAtPosition(position, regex) {
        const line = this._lines[position.line] || '';
        const pattern = regex || /[a-zA-Z_]\w*/g;
        pattern.lastIndex = 0;
        let match;
        while ((match = pattern.exec(line)) !== null) {
            const start = match.index;
            const end = start + match[0].length;
            if (start <= position.character && position.character <= end) {
                return new Range(position.line, start, position.line, end);
            }
        }
        return undefined;
    }
    validateRange(range) { return range; }
    validatePosition(position) { return position; }
    save() { return rpc.request('mainThread/saveDocument', { uri: this._uri.toString() }); }
}

// ─── Output Channel ────────────────────────────────────────────────────────────

class OutputChannel {
    constructor(name) { this._name = name; this._content = ''; }
    get name() { return this._name; }
    append(value) { this._content += value; rpc.notify('mainThread/outputAppend', { name: this._name, value }); }
    appendLine(value) { this.append(value + '\n'); }
    replace(value) { this._content = value; rpc.notify('mainThread/outputReplace', { name: this._name, value }); }
    clear() { this._content = ''; rpc.notify('mainThread/outputClear', { name: this._name }); }
    show(preserveFocus) { rpc.notify('mainThread/outputShow', { name: this._name, preserveFocus: preserveFocus ?? false }); }
    hide() { rpc.notify('mainThread/outputHide', { name: this._name }); }
    dispose() { rpc.notify('mainThread/outputDispose', { name: this._name }); }
}

// ─── Diagnostic Collection ─────────────────────────────────────────────────────

class DiagnosticCollection {
    constructor(name) { this._name = name; this._entries = new Map(); }
    get name() { return this._name; }
    set(uri, diagnostics) {
        if (diagnostics) {
            this._entries.set(uri.toString(), { uri, diagnostics });
        } else {
            this._entries.delete(uri.toString());
        }
        rpc.notify('mainThread/setDiagnostics', {
            source: this._name,
            uri: uri.toString(),
            diagnostics: (diagnostics || []).map(d => ({
                range: { start: { line: d.range.start.line, character: d.range.start.character }, end: { line: d.range.end.line, character: d.range.end.character } },
                message: d.message, severity: d.severity, source: d.source, code: d.code
            }))
        });
    }
    delete(uri) { this.set(uri, undefined); }
    has(uri) { return this._entries.has(uri.toString()); }
    get(uri) { const e = this._entries.get(uri.toString()); return e ? e.diagnostics : undefined; }
    clear() { for (const k of this._entries.keys()) this.set(Uri.parse(k), undefined); this._entries.clear(); }
    forEach(callback) { this._entries.forEach((v) => callback(v.uri, v.diagnostics, this)); }
    dispose() { this.clear(); diagnosticCollections.delete(this._name); }
}

// ─── Status Bar Item ───────────────────────────────────────────────────────────

class StatusBarItem {
    constructor(id, alignment, priority) {
        this._id = id; this.alignment = alignment; this.priority = priority;
        this.text = ''; this.tooltip = ''; this.color = undefined; this.backgroundColor = undefined;
        this.command = undefined; this.accessibilityInformation = undefined;
        this._visible = false; this.name = '';
    }
    show() { this._visible = true; rpc.notify('mainThread/statusBarShow', { id: this._id, text: this.text, tooltip: this.tooltip, alignment: this.alignment, priority: this.priority, command: this.command }); }
    hide() { this._visible = false; rpc.notify('mainThread/statusBarHide', { id: this._id }); }
    dispose() { this.hide(); }
}
let statusBarIdCounter = 0;

// ─── vscode.* Namespace Factory ────────────────────────────────────────────────

const onDidChangeActiveTextEditorEmitter = new EventEmitter();
const onDidChangeVisibleTextEditorsEmitter = new EventEmitter();
const onDidChangeTextEditorSelectionEmitter = new EventEmitter();
const onDidOpenTextDocumentEmitter = new EventEmitter();
const onDidCloseTextDocumentEmitter = new EventEmitter();
const onDidChangeTextDocumentEmitter = new EventEmitter();
const onDidSaveTextDocumentEmitter = new EventEmitter();
const onDidChangeConfigurationEmitter = new EventEmitter();
const onDidChangeWorkspaceFoldersEmitter = new EventEmitter();

function createVscodeApi(extensionId) {
    const vscode = {
        // Types
        Position, Range, Selection, Uri, Disposable, EventEmitter, CancellationTokenSource,
        TextEdit, WorkspaceEdit, Location, Hover, MarkdownString, SnippetString,
        Diagnostic, DiagnosticSeverity, CompletionItem, CompletionItemKind, CompletionList,
        SymbolKind, SymbolInformation, DocumentSymbol, ThemeColor, ThemeIcon, TreeItem,
        CodeAction, CodeActionKind, CodeLens, FoldingRange, FoldingRangeKind,
        SemanticTokensLegend, SemanticTokensBuilder, SignatureHelp, SignatureInformation, ParameterInformation,
        StatusBarAlignment, ViewColumn, EndOfLine, TextEditorRevealType,
        ConfigurationTarget, TextDocumentSaveReason, FileType, TreeItemCollapsibleState,
        ProgressLocation, IndentAction, DecorationRangeBehavior, OverviewRulerLane,

        // commands namespace
        commands: {
            registerCommand(id, handler, thisArg) {
                const bound = thisArg ? handler.bind(thisArg) : handler;
                commandHandlers.set(id, bound);
                rpc.notify('mainThread/registerCommand', { id });
                return new Disposable(() => { commandHandlers.delete(id); rpc.notify('mainThread/unregisterCommand', { id }); });
            },
            registerTextEditorCommand(id, handler, thisArg) {
                return vscode.commands.registerCommand(id, (...args) => {
                    const editor = activeTextEditor;
                    if (editor) return handler.call(thisArg, editor, editor._edit, ...args);
                });
            },
            executeCommand(id, ...args) { return rpc.request('mainThread/executeCommand', { id, args }); },
            getCommands(filterInternal) { return rpc.request('mainThread/getCommands', { filterInternal: filterInternal ?? false }); },
        },

        // window namespace
        window: {
            get activeTextEditor() { return activeTextEditor; },
            get visibleTextEditors() { return visibleTextEditors; },
            onDidChangeActiveTextEditor: onDidChangeActiveTextEditorEmitter.event,
            onDidChangeVisibleTextEditors: onDidChangeVisibleTextEditorsEmitter.event,
            onDidChangeTextEditorSelection: onDidChangeTextEditorSelectionEmitter.event,
            showInformationMessage(message, ...items) { return rpc.request('mainThread/showMessage', { severity: 'info', message, items }); },
            showWarningMessage(message, ...items) { return rpc.request('mainThread/showMessage', { severity: 'warning', message, items }); },
            showErrorMessage(message, ...items) { return rpc.request('mainThread/showMessage', { severity: 'error', message, items }); },
            showInputBox(options) { return rpc.request('mainThread/showInputBox', options || {}); },
            showQuickPick(items, options) { return rpc.request('mainThread/showQuickPick', { items: Array.isArray(items) ? items : [], ...(options || {}) }); },
            showOpenDialog(options) { return rpc.request('mainThread/showOpenDialog', options || {}); },
            showSaveDialog(options) { return rpc.request('mainThread/showSaveDialog', options || {}); },
            createOutputChannel(name) { return new OutputChannel(name); },
            createTerminal(nameOrOptions) {
                const opts = typeof nameOrOptions === 'string' ? { name: nameOrOptions } : (nameOrOptions || {});
                rpc.notify('mainThread/createTerminal', opts);
                return { show() {}, sendText(text) { rpc.notify('mainThread/terminalSendText', { name: opts.name, text }); }, dispose() {} };
            },
            showTextDocument(uriOrDoc, options) {
                const uri = uriOrDoc instanceof Uri ? uriOrDoc : uriOrDoc.uri;
                return rpc.request('mainThread/showTextDocument', { uri: uri.toString(), ...(options || {}) });
            },
            createStatusBarItem(alignmentOrId, priorityOrAlignment, priority) {
                let id, alignment, prio;
                if (typeof alignmentOrId === 'string') { id = alignmentOrId; alignment = priorityOrAlignment; prio = priority; }
                else { id = `statusbar_${++statusBarIdCounter}`; alignment = alignmentOrId; prio = priorityOrAlignment; }
                return new StatusBarItem(id, alignment ?? StatusBarAlignment.Left, prio ?? 0);
            },
            createTreeView(viewId, options) {
                treeDataProviders.set(viewId, options.treeDataProvider);
                rpc.notify('mainThread/registerTreeView', { viewId });
                return { reveal() {}, dispose() { treeDataProviders.delete(viewId); } };
            },
            registerTreeDataProvider(viewId, provider) {
                treeDataProviders.set(viewId, provider);
                rpc.notify('mainThread/registerTreeView', { viewId });
                return new Disposable(() => treeDataProviders.delete(viewId));
            },
            setStatusBarMessage(text, hideAfterOrTimeout) {
                rpc.notify('mainThread/setStatusBarMessage', { text });
                const timeout = typeof hideAfterOrTimeout === 'number' ? hideAfterOrTimeout : 5000;
                const d = new Disposable(() => rpc.notify('mainThread/clearStatusBarMessage', {}));
                setTimeout(() => d.dispose(), timeout);
                return d;
            },
            withProgress(options, task) {
                const progress = { report(value) { rpc.notify('mainThread/progressReport', { ...options, ...value }); } };
                const cts = new CancellationTokenSource();
                return task(progress, cts.token);
            },
            createTextEditorDecorationType(options) {
                const key = `deco_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerDecorationType', { key, options });
                return { key, dispose() { rpc.notify('mainThread/removeDecorationType', { key }); } };
            },
            registerWebviewViewProvider(viewId, provider) {
                rpc.notify('mainThread/registerWebviewView', { viewId });
                return new Disposable(() => {});
            },
        },

        // workspace namespace
        workspace: {
            get workspaceFolders() { return workspaceFolders; },
            get name() { return workspaceFolders.length > 0 ? path.basename(workspaceFolders[0].uri.fsPath) : undefined; },
            get rootPath() { return workspaceFolders.length > 0 ? workspaceFolders[0].uri.fsPath : undefined; },
            onDidChangeWorkspaceFolders: onDidChangeWorkspaceFoldersEmitter.event,
            onDidOpenTextDocument: onDidOpenTextDocumentEmitter.event,
            onDidCloseTextDocument: onDidCloseTextDocumentEmitter.event,
            onDidChangeTextDocument: onDidChangeTextDocumentEmitter.event,
            onDidSaveTextDocument: onDidSaveTextDocumentEmitter.event,
            onDidChangeConfiguration: onDidChangeConfigurationEmitter.event,
            getConfiguration(section, scope) {
                return {
                    get(key, defaultValue) { return configCache.get(`${section}.${key}`) ?? defaultValue; },
                    has(key) { return configCache.has(`${section}.${key}`); },
                    inspect(key) { return { key: `${section}.${key}`, defaultValue: undefined, globalValue: configCache.get(`${section}.${key}`), workspaceValue: undefined }; },
                    update(key, value, target) { return rpc.request('mainThread/updateConfiguration', { section, key, value, target: target ?? ConfigurationTarget.Global }); }
                };
            },
            openTextDocument(uriOrOpts) {
                const uri = uriOrOpts instanceof Uri ? uriOrOpts : (typeof uriOrOpts === 'string' ? Uri.file(uriOrOpts) : undefined);
                return rpc.request('mainThread/openTextDocument', { uri: uri ? uri.toString() : undefined, ...(typeof uriOrOpts === 'object' && !(uriOrOpts instanceof Uri) ? uriOrOpts : {}) })
                    .then(doc => new TextDocumentProxy(doc.uri, doc.languageId, doc.version, doc.content));
            },
            findFiles(include, exclude, maxResults, token) {
                return rpc.request('mainThread/findFiles', { include: include.toString(), exclude: exclude ? exclude.toString() : undefined, maxResults })
                    .then(uris => uris.map(u => Uri.parse(u)));
            },
            createFileSystemWatcher(pattern, ignoreCreate, ignoreChange, ignoreDelete) {
                const emitters = { create: new EventEmitter(), change: new EventEmitter(), delete: new EventEmitter() };
                rpc.notify('mainThread/watchFiles', { pattern: pattern.toString() });
                return {
                    onDidCreate: emitters.create.event, onDidChange: emitters.change.event, onDidDelete: emitters.delete.event,
                    dispose() { rpc.notify('mainThread/unwatchFiles', { pattern: pattern.toString() }); }
                };
            },
            applyEdit(edit) { return rpc.request('mainThread/applyWorkspaceEdit', { edit: serializeWorkspaceEdit(edit) }); },
            registerTextDocumentContentProvider(scheme, provider) {
                rpc.notify('mainThread/registerContentProvider', { scheme });
                return new Disposable(() => rpc.notify('mainThread/unregisterContentProvider', { scheme }));
            },
            getWorkspaceFolder(uri) {
                for (const f of workspaceFolders) { if (uri.fsPath.startsWith(f.uri.fsPath)) return f; }
                return undefined;
            },
            asRelativePath(pathOrUri, includeWorkspaceFolder) {
                const p = typeof pathOrUri === 'string' ? pathOrUri : pathOrUri.fsPath;
                for (const f of workspaceFolders) { if (p.startsWith(f.uri.fsPath)) return path.relative(f.uri.fsPath, p); }
                return p;
            },
            fs: {
                readFile(uri) { return rpc.request('mainThread/fsReadFile', { uri: uri.toString() }).then(data => Buffer.from(data, 'base64')); },
                writeFile(uri, content) { return rpc.request('mainThread/fsWriteFile', { uri: uri.toString(), content: Buffer.from(content).toString('base64') }); },
                stat(uri) { return rpc.request('mainThread/fsStat', { uri: uri.toString() }); },
                readDirectory(uri) { return rpc.request('mainThread/fsReadDir', { uri: uri.toString() }); },
                createDirectory(uri) { return rpc.request('mainThread/fsCreateDir', { uri: uri.toString() }); },
                delete(uri, options) { return rpc.request('mainThread/fsDelete', { uri: uri.toString(), ...(options || {}) }); },
                rename(source, target, options) { return rpc.request('mainThread/fsRename', { source: source.toString(), target: target.toString(), ...(options || {}) }); },
                copy(source, target, options) { return rpc.request('mainThread/fsCopy', { source: source.toString(), target: target.toString(), ...(options || {}) }); },
            },
        },

        // languages namespace
        languages: {
            registerCompletionItemProvider(selector, provider, ...triggerChars) {
                const id = `completion_${++statusBarIdCounter}`;
                completionProviders.set(id, { selector, provider, triggerChars });
                rpc.notify('mainThread/registerCompletionProvider', { id, selector: normalizeSelector(selector), triggerChars });
                return new Disposable(() => { completionProviders.delete(id); rpc.notify('mainThread/unregisterProvider', { id }); });
            },
            registerHoverProvider(selector, provider) {
                const id = `hover_${++statusBarIdCounter}`;
                hoverProviders.set(id, { selector, provider });
                rpc.notify('mainThread/registerHoverProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => { hoverProviders.delete(id); rpc.notify('mainThread/unregisterProvider', { id }); });
            },
            registerDefinitionProvider(selector, provider) {
                const id = `definition_${++statusBarIdCounter}`;
                definitionProviders.set(id, { selector, provider });
                rpc.notify('mainThread/registerDefinitionProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => { definitionProviders.delete(id); rpc.notify('mainThread/unregisterProvider', { id }); });
            },
            registerCodeActionsProvider(selector, provider, metadata) {
                const id = `codeaction_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerCodeActionsProvider', { id, selector: normalizeSelector(selector), metadata });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerDocumentFormattingEditProvider(selector, provider) {
                const id = `format_${++statusBarIdCounter}`;
                formatProviders.set(id, { selector, provider });
                rpc.notify('mainThread/registerFormattingProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => { formatProviders.delete(id); rpc.notify('mainThread/unregisterProvider', { id }); });
            },
            registerDocumentRangeFormattingEditProvider(selector, provider) {
                const id = `rangeformat_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerRangeFormattingProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerDocumentSymbolProvider(selector, provider) {
                const id = `symbol_${++statusBarIdCounter}`;
                symbolProviders.set(id, { selector, provider });
                rpc.notify('mainThread/registerDocumentSymbolProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => { symbolProviders.delete(id); rpc.notify('mainThread/unregisterProvider', { id }); });
            },
            registerReferenceProvider(selector, provider) {
                const id = `reference_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerReferenceProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerRenameProvider(selector, provider) {
                const id = `rename_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerRenameProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerSignatureHelpProvider(selector, provider, ...triggerChars) {
                const id = `sighelp_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerSignatureHelpProvider', { id, selector: normalizeSelector(selector), triggerChars });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerDocumentHighlightProvider(selector, provider) {
                const id = `highlight_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerHighlightProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerDocumentLinkProvider(selector, provider) {
                const id = `doclink_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerDocumentLinkProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerColorProvider(selector, provider) {
                const id = `color_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerColorProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerFoldingRangeProvider(selector, provider) {
                const id = `folding_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerFoldingRangeProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerCodeLensProvider(selector, provider) {
                const id = `codelens_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerCodeLensProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerDocumentSemanticTokensProvider(selector, provider, legend) {
                const id = `semtokens_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerSemanticTokensProvider', { id, selector: normalizeSelector(selector), legend: { tokenTypes: legend.tokenTypes, tokenModifiers: legend.tokenModifiers } });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerOnTypeFormattingEditProvider(selector, provider, firstTriggerChar, ...moreTriggerChar) {
                const id = `ontype_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerOnTypeFormattingProvider', { id, selector: normalizeSelector(selector), triggerChars: [firstTriggerChar, ...moreTriggerChar] });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerSelectionRangeProvider(selector, provider) {
                const id = `selrange_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerSelectionRangeProvider', { id, selector: normalizeSelector(selector) });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            registerWorkspaceSymbolProvider(provider) {
                const id = `wssymbol_${++statusBarIdCounter}`;
                rpc.notify('mainThread/registerWorkspaceSymbolProvider', { id });
                return new Disposable(() => rpc.notify('mainThread/unregisterProvider', { id }));
            },
            createDiagnosticCollection(name) {
                const collection = new DiagnosticCollection(name || `diag_${++statusBarIdCounter}`);
                diagnosticCollections.set(collection.name, collection);
                return collection;
            },
            setTextDocumentLanguage(document, languageId) { return rpc.request('mainThread/setLanguage', { uri: document.uri.toString(), languageId }); },
            getLanguages() { return rpc.request('mainThread/getLanguages', {}); },
            match(selector, document) {
                const sel = normalizeSelector(selector);
                return sel.some(s => !s.language || s.language === document.languageId) ? 10 : 0;
            },
            getDiagnostics(uri) {
                if (uri) { for (const c of diagnosticCollections.values()) { const d = c.get(uri); if (d) return d; } return []; }
                const all = []; diagnosticCollections.forEach(c => c.forEach((u, d) => all.push([u, d]))); return all;
            },
        },

        // env namespace
        env: {
            appName: 'vsedit',
            appRoot: process.cwd(),
            language: process.env.LANG || 'en',
            machineId: 'unknown',
            sessionId: `session_${Date.now()}`,
            uriScheme: 'vsedit',
            clipboard: {
                readText() { return rpc.request('mainThread/clipboardRead', {}); },
                writeText(value) { return rpc.request('mainThread/clipboardWrite', { value }); }
            },
            openExternal(uri) { return rpc.request('mainThread/openExternal', { uri: uri.toString() }); },
        },

        // extensions namespace
        extensions: {
            getExtension(id) { return extensions.get(id) || undefined; },
            get all() { return Array.from(extensions.values()); },
            onDidChange: new EventEmitter().event,
        },

        // debug namespace
        debug: {
            get activeDebugSession() { return undefined; },
            get activeDebugConsole() { return { append() {}, appendLine() {} }; },
            get breakpoints() { return []; },
            onDidChangeActiveDebugSession: new EventEmitter().event,
            onDidStartDebugSession: new EventEmitter().event,
            onDidTerminateDebugSession: new EventEmitter().event,
            onDidChangeBreakpoints: new EventEmitter().event,
            onDidReceiveDebugSessionCustomEvent: new EventEmitter().event,
            registerDebugConfigurationProvider(type, provider) { return new Disposable(() => {}); },
            registerDebugAdapterDescriptorFactory(type, factory) { return new Disposable(() => {}); },
            startDebugging(folder, nameOrConfig, options) { return rpc.request('mainThread/startDebugging', { config: nameOrConfig }); },
            stopDebugging(session) { return rpc.request('mainThread/stopDebugging', {}); },
            addBreakpoints(breakpoints) { rpc.notify('mainThread/addBreakpoints', { breakpoints }); },
            removeBreakpoints(breakpoints) { rpc.notify('mainThread/removeBreakpoints', { breakpoints }); },
        },

        // tasks namespace
        tasks: {
            registerTaskProvider(type, provider) { return new Disposable(() => {}); },
            fetchTasks(filter) { return rpc.request('mainThread/fetchTasks', { filter }); },
            executeTask(task) { return rpc.request('mainThread/executeTask', { task }); },
            onDidStartTask: new EventEmitter().event,
            onDidEndTask: new EventEmitter().event,
            onDidStartTaskProcess: new EventEmitter().event,
            onDidEndTaskProcess: new EventEmitter().event,
        },

        // scm namespace
        scm: {
            createSourceControl(id, label, rootUri) {
                rpc.notify('mainThread/registerSourceControl', { id, label, rootUri: rootUri ? rootUri.toString() : undefined });
                const inputBox = { value: '', placeholder: '' };
                return {
                    id, label, inputBox, rootUri,
                    createResourceGroup(id, label) {
                        return { id, label, resourceStates: [], dispose() {} };
                    },
                    dispose() { rpc.notify('mainThread/unregisterSourceControl', { id }); }
                };
            }
        },
    };

    return vscode;
}

// ─── Provider Registries ───────────────────────────────────────────────────────

const completionProviders = new Map();
const hoverProviders = new Map();
const definitionProviders = new Map();
const formatProviders = new Map();
const symbolProviders = new Map();

function normalizeSelector(selector) {
    if (typeof selector === 'string') return [{ language: selector }];
    if (Array.isArray(selector)) return selector.map(s => typeof s === 'string' ? { language: s } : s);
    return [selector];
}

function serializeWorkspaceEdit(edit) {
    const entries = [];
    edit.entries().forEach(([uri, edits]) => {
        entries.push({ uri: uri.toString(), edits: edits.map(e => ({ range: e.range, newText: e.newText })) });
    });
    return { entries };
}

// ─── State ─────────────────────────────────────────────────────────────────────

let activeTextEditor = undefined;
let visibleTextEditors = [];
let workspaceFolders = [];
const configCache = new Map();

// ─── Extension Loading ─────────────────────────────────────────────────────────

function createExtensionContext(extensionId, extensionPath) {
    const storagePath = path.join(extensionPath, '.storage');
    const globalStoragePath = path.join(process.env.HOME || '/tmp', '.config', 'vsedit', 'globalStorage', extensionId);

    const subscriptions = [];
    const workspaceState = createMemento(`ws:${extensionId}`);
    const globalState = createMemento(`gs:${extensionId}`);

    return {
        subscriptions,
        extensionPath,
        extensionUri: Uri.file(extensionPath),
        storagePath,
        storageUri: Uri.file(storagePath),
        globalStoragePath,
        globalStorageUri: Uri.file(globalStoragePath),
        logPath: path.join(storagePath, 'logs'),
        logUri: Uri.file(path.join(storagePath, 'logs')),
        extensionMode: 1, // Production
        extension: extensions.get(extensionId),
        asAbsolutePath(relativePath) { return path.join(extensionPath, relativePath); },
        workspaceState,
        globalState,
        secrets: {
            get(key) { return rpc.request('mainThread/secretGet', { extensionId, key }); },
            store(key, value) { return rpc.request('mainThread/secretStore', { extensionId, key, value }); },
            delete(key) { return rpc.request('mainThread/secretDelete', { extensionId, key }); },
            onDidChange: new EventEmitter().event,
        },
        environmentVariableCollection: {
            persistent: true,
            replace(variable, value) {},
            append(variable, value) {},
            prepend(variable, value) {},
            get(variable) { return undefined; },
            forEach(callback) {},
            delete(variable) {},
            clear() {},
        },
    };
}

function createMemento(prefix) {
    const store = new Map();
    return {
        keys() { return Array.from(store.keys()); },
        get(key, defaultValue) { return store.has(key) ? store.get(key) : defaultValue; },
        update(key, value) {
            if (value === undefined) store.delete(key);
            else store.set(key, value);
            return rpc.request('mainThread/mementoUpdate', { prefix, key, value });
        },
        setKeysForSync(keys) {},
    };
}

// ─── Host-to-Extension RPC Handlers ────────────────────────────────────────────

// Execute a command registered by an extension
rpc.onRequest('ext/executeCommand', async (params) => {
    const { id, args } = params;
    const handler = commandHandlers.get(id);
    if (!handler) throw new Error(`Command not found: ${id}`);
    return await handler(...(args || []));
});

// Activate an extension
rpc.onRequest('ext/activate', async (params) => {
    const { extensionId, extensionPath, packageJSON } = params;
    try {
        const mainFile = packageJSON.main || 'extension.js';
        const mainPath = path.resolve(extensionPath, mainFile);
        if (!fs.existsSync(mainPath)) throw new Error(`Extension main file not found: ${mainPath}`);

        const vscode = createVscodeApi(extensionId);
        // Inject vscode module into require resolution
        const Module = require('module');
        const origResolve = Module._resolveFilename;
        Module._resolveFilename = function(request, parent, isMain, options) {
            if (request === 'vscode') return 'vscode';
            return origResolve.call(this, request, parent, isMain, options);
        };
        const origLoad = Module._cache;
        require.cache['vscode'] = { id: 'vscode', filename: 'vscode', loaded: true, exports: vscode };

        const ext = require(mainPath);
        extensions.set(extensionId, { id: extensionId, extensionPath, extensionUri: Uri.file(extensionPath), isActive: true, packageJSON, exports: ext });

        if (typeof ext.activate === 'function') {
            const context = createExtensionContext(extensionId, extensionPath);
            await ext.activate(context);
        }
        return { success: true };
    } catch (e) {
        return { success: false, error: e.message, stack: e.stack };
    }
});

// Deactivate an extension
rpc.onRequest('ext/deactivate', async (params) => {
    const { extensionId } = params;
    const ext = extensions.get(extensionId);
    if (ext && ext.exports && typeof ext.exports.deactivate === 'function') {
        await ext.exports.deactivate();
    }
    extensions.delete(extensionId);
    return { success: true };
});

// Provider invocations from host
rpc.onRequest('ext/provideCompletionItems', async (params) => {
    const { providerId, uri, position, context } = params;
    const entry = completionProviders.get(providerId);
    if (!entry) return { items: [] };
    const doc = await getOrFetchDocument(uri);
    const pos = new Position(position.line, position.character);
    const result = await entry.provider.provideCompletionItems(doc, pos, { isCancellationRequested: false, onCancellationRequested: new EventEmitter().event }, context || {});
    if (!result) return { items: [] };
    const items = Array.isArray(result) ? result : result.items;
    return { items: items.map(serializeCompletionItem), isIncomplete: result.isIncomplete || false };
});

rpc.onRequest('ext/provideHover', async (params) => {
    const { providerId, uri, position } = params;
    const entry = hoverProviders.get(providerId);
    if (!entry) return null;
    const doc = await getOrFetchDocument(uri);
    const pos = new Position(position.line, position.character);
    const result = await entry.provider.provideHover(doc, pos, { isCancellationRequested: false, onCancellationRequested: new EventEmitter().event });
    if (!result) return null;
    return { contents: result.contents.map(c => typeof c === 'string' ? c : c.value), range: result.range };
});

rpc.onRequest('ext/provideDefinition', async (params) => {
    const { providerId, uri, position } = params;
    const entry = definitionProviders.get(providerId);
    if (!entry) return null;
    const doc = await getOrFetchDocument(uri);
    const pos = new Position(position.line, position.character);
    const result = await entry.provider.provideDefinition(doc, pos, { isCancellationRequested: false, onCancellationRequested: new EventEmitter().event });
    if (!result) return null;
    if (Array.isArray(result)) return result.map(serializeLocation);
    if (result.uri) return [serializeLocation(result)];
    return [serializeLocation(result)];
});

rpc.onRequest('ext/provideDocumentSymbols', async (params) => {
    const { providerId, uri } = params;
    const entry = symbolProviders.get(providerId);
    if (!entry) return [];
    const doc = await getOrFetchDocument(uri);
    const result = await entry.provider.provideDocumentSymbols(doc, { isCancellationRequested: false, onCancellationRequested: new EventEmitter().event });
    return result || [];
});

rpc.onRequest('ext/provideDocumentFormattingEdits', async (params) => {
    const { providerId, uri, options } = params;
    const entry = formatProviders.get(providerId);
    if (!entry) return [];
    const doc = await getOrFetchDocument(uri);
    const result = await entry.provider.provideDocumentFormattingEdits(doc, options || {}, { isCancellationRequested: false, onCancellationRequested: new EventEmitter().event });
    return (result || []).map(e => ({ range: e.range, newText: e.newText }));
});

// Notify extension of document events
rpc.onNotification('ext/textDocumentDidOpen', (params) => {
    const doc = new TextDocumentProxy(params.uri, params.languageId, params.version, params.content);
    documentCache.set(params.uri, doc);
    onDidOpenTextDocumentEmitter.fire(doc);
});

rpc.onNotification('ext/textDocumentDidChange', (params) => {
    const doc = documentCache.get(params.uri);
    if (doc) {
        doc._version = params.version;
        doc._content = params.content;
        doc._lines = params.content.split(/\r?\n/);
        onDidChangeTextDocumentEmitter.fire({ document: doc, contentChanges: params.changes || [] });
    }
});

rpc.onNotification('ext/textDocumentDidClose', (params) => {
    const doc = documentCache.get(params.uri);
    if (doc) { documentCache.delete(params.uri); onDidCloseTextDocumentEmitter.fire(doc); }
});

rpc.onNotification('ext/textDocumentDidSave', (params) => {
    const doc = documentCache.get(params.uri);
    if (doc) onDidSaveTextDocumentEmitter.fire(doc);
});

rpc.onNotification('ext/configurationChanged', (params) => {
    if (params.settings) {
        for (const [k, v] of Object.entries(params.settings)) configCache.set(k, v);
    }
    onDidChangeConfigurationEmitter.fire({ affectsConfiguration(section) { return true; } });
});

rpc.onNotification('ext/workspaceFoldersChanged', (params) => {
    workspaceFolders = (params.folders || []).map((f, i) => ({ uri: Uri.parse(f.uri), name: f.name || path.basename(f.uri), index: i }));
    onDidChangeWorkspaceFoldersEmitter.fire({ added: workspaceFolders, removed: [] });
});

rpc.onNotification('ext/activeEditorChanged', (params) => {
    if (params.uri) {
        const doc = documentCache.get(params.uri) || new TextDocumentProxy(params.uri, params.languageId || 'plaintext', 1, params.content || '');
        activeTextEditor = { document: doc, selection: new Selection(0, 0, 0, 0), selections: [new Selection(0, 0, 0, 0)], options: {}, viewColumn: ViewColumn.One };
    } else {
        activeTextEditor = undefined;
    }
    onDidChangeActiveTextEditorEmitter.fire(activeTextEditor);
});

// ─── Document Cache ────────────────────────────────────────────────────────────

const documentCache = new Map();

async function getOrFetchDocument(uri) {
    if (documentCache.has(uri)) return documentCache.get(uri);
    const doc = await rpc.request('mainThread/openTextDocument', { uri });
    const proxy = new TextDocumentProxy(doc.uri, doc.languageId, doc.version, doc.content);
    documentCache.set(uri, proxy);
    return proxy;
}

function serializeCompletionItem(item) {
    return { label: item.label, kind: item.kind, detail: item.detail, documentation: item.documentation, insertText: typeof item.insertText === 'string' ? item.insertText : item.insertText?.value, sortText: item.sortText, filterText: item.filterText, preselect: item.preselect };
}

function serializeLocation(loc) {
    return { uri: loc.uri.toString(), range: { start: { line: loc.range.start.line, character: loc.range.start.character }, end: { line: loc.range.end.line, character: loc.range.end.character } } };
}

// ─── Start ─────────────────────────────────────────────────────────────────────

rpc.start();
rpc.notify('ext/ready', { pid: process.pid, version: '1.0.0' });
