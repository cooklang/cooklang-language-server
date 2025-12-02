import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    // Get server path from settings or search in PATH
    const config = vscode.workspace.getConfiguration('cooklang');
    let serverPath = config.get<string>('serverPath');

    if (!serverPath) {
        // Default: assume cooklang-lsp is in PATH or use bundled
        serverPath = 'cooklang-lsp';
    }

    const serverOptions: ServerOptions = {
        run: {
            command: serverPath,
            args: [],
        },
        debug: {
            command: serverPath,
            args: [],
        },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'cooklang' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.cook'),
        },
    };

    client = new LanguageClient(
        'cooklang',
        'Cooklang Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
