export interface ParsedRule {
    init: number;
    step: 1 | -1;
    len: number;
}
export declare const getRuleString: (out: string) => string | null;
export declare const buildRule: (rule: string) => ParsedRule | null;
export declare const buildOuts: (uris: string[], out: string) => string[];
