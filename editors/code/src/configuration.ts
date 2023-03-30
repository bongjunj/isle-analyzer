// Copyright (c) The Diem Core Contributors
// Copyright (c) The ISLE Contributors
// SPDX-License-Identifier: Apache-2.0

import * as os from 'os';
import * as vscode from 'vscode';
import * as Path from 'path';

/**
 * User-defined configuration values, such as those specified in VS Code settings.
 *
 * This provides a more strongly typed interface to the configuration values specified in this
 * extension's `package.json`, under the key `"contributes.configuration.properties"`.
 */
const defaultName = 'isle-analyzer';


export class Configuration {
    private readonly configuration: vscode.WorkspaceConfiguration;

    constructor() {
        this.configuration = vscode.workspace.getConfiguration('isle-analyzer');
    }

    /** A string representation of the configured values, for logging purposes. */
    toString(): string {
        return JSON.stringify(this.configuration);
    }

    /** The path to the isle-analyzer executable. */
    get serverPath(): string {

        let serverPath = this.configuration.get<string>('server.path', defaultName);
        if (serverPath.length === 0) {
            // The default value of the `server.path` setting is 'isle-analyzer'.
            // A user may have over-written this default with an empty string value, ''.
            // An empty string cannot be an executable name, so instead use the default.
            return defaultName;
        }

        if (serverPath === defaultName) {
            // If the program set by the user is through PATH,
            // it will return directly if specified
            return defaultName;
        }

        if (serverPath.startsWith('~/')) {
            serverPath = os.homedir() + serverPath.slice('~'.length);
        }

        if (process.platform === 'win32' && !serverPath.endsWith('.exe')) {
            serverPath = serverPath + '.exe';
        }

        return Path.resolve(serverPath);
    }

    externalDependencies(): string[] {
        const d = this.configuration.get<string[]>('external.dependencies');
        if (d === undefined) {
            return [];
        }
        return d;
    }
}
