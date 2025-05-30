/* ----------------------------------------------------------------------- *
 *
 *   Copyright 1996-2024 The NASM Authors - All Rights Reserved
 *   See the file AUTHORS included with the NASM distribution for
 *   the specific copyright holders.
 *
 *   Redistribution and use in source and binary forms, with or without
 *   modification, are permitted provided that the following
 *   conditions are met:
 *
 *   * Redistributions of source code must retain the above copyright
 *     notice, this list of conditions and the following disclaimer.
 *   * Redistributions in binary form must reproduce the above
 *     copyright notice, this list of conditions and the following
 *     disclaimer in the documentation and/or other materials provided
 *     with the distribution.
 *
 *     THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 *     CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 *     INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 *     MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 *     DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 *     CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 *     SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 *     NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
 *     LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 *     HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 *     CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
 *     OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE,
 *     EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * ----------------------------------------------------------------------- */

/*
 * preproc.c   macro preprocessor for the Netwide Assembler
 */

/* Typical flow of text through preproc
 *
 * pp_getline gets tokenized lines, either
 *
 *   from a macro expansion
 *
 * or
 *   {
 *   read_line  gets raw text from stdmacpos, or predef, or current input file
 *   tokenize   converts to tokens
 *   }
 *
 * expand_mmac_params is used to expand %1 etc., unless a macro is being
 * defined or a false conditional is being processed
 * (%0, %1, %+1, %-1, %%foo
 *
 * do_directive checks for directives
 *
 * expand_smacro is used to expand single line macros
 *
 * expand_mmacro is used to expand multi-line macros
 *
 * detoken is used to convert the line back to text
 */

#include "compiler.h"

#include "nctype.h"

#include "nasm.h"
#include "nasmlib.h"
#include "error.h"
#include "preproc.h"
#include "hashtbl.h"
#include "quote.h"
#include "stdscan.h"
#include "eval.h"
#include "tokens.h"
#include "tables.h"
#include "listing.h"
#include "dbginfo.h"

/*
 * Preprocessor execution options that can be controlled by %pragma or
 * other directives.  This structure is initialized to zero on each
 * pass; this *must* reflect the default initial state.
 */
static struct pp_config {
    bool noaliases;
    bool sane_empty_expansion;
} ppconf;

/*
 * Preprocessor debug-related flags
 */
static enum pp_debug_flags {
    PDBG_MMACROS      = 1,      /* Collect mmacro information */
    PDBG_SMACROS      = 2,      /* Collect smacro information */
    PDBG_LIST_SMACROS = 4,      /* Smacros to list file (list option 's') */
    PDBG_INCLUDE      = 8       /* Collect %include information */
} ppdbg;

/*
 * Preprocessor options configured on the command line
 */
static enum preproc_opt ppopt;

typedef struct SMacro SMacro;
typedef struct MMacro MMacro;
typedef struct Context Context;
typedef struct Token Token;
typedef struct Line Line;
typedef struct Include Include;
typedef struct Cond Cond;

/*
 * Map of preprocessor directives that are also preprocessor functions;
 * if they are at the beginning of a line they are a function if and
 * only if they are followed by a (
 */
static bool pp_op_may_be_function[PP_count];

/*
 * This is the internal form which we break input lines up into.
 * Typically stored in linked lists.
 *
 * Note that `type' serves a double meaning: TOKEN_SMAC_START_PARAMS is
 * not necessarily used as-is, but is also used to encode the number
 * and expansion type of substituted parameter. So in the definition
 *
 *     %define a(x,=y) ( (x) & ~(y) )
 *
 * the token representing `x' will have its type changed to
 * tok_smac_param(0) but the one representing `y' will be
 * tok_smac_param(1); see the accessor functions below.
 *
 * TOKEN_INTERNAL_STR is a string which has been unquoted, but should
 * be treated as if it was a quoted string. The code is free to change
 * one into the other at will. TOKEN_NAKED_STR is a text token which
 * should be treated as a string, but which MUST NOT be turned into a
 * quoted string. TOKEN_INTERNAL_STRs can contain any character,
 * including NUL, but TOKEN_NAKED_STR must be a valid C string.
 */

static inline enum token_type tok_smac_param(int param)
{
    return TOKEN_SMAC_START_PARAMS + param;
}
static int smac_nparam(enum token_type toktype)
{
    return toktype - TOKEN_SMAC_START_PARAMS;
}
static bool is_smac_param(enum token_type toktype)
{
    return toktype >= TOKEN_SMAC_START_PARAMS;
}

/*
 * This is tuned so struct Token should be 64 bytes on 64-bit
 * systems and 32 bytes on 32-bit systems. It enables them
 * to be nicely cache aligned, and the text to still be kept
 * inline for nearly all tokens.
 *
 * We prohibit tokens of length > MAX_TEXT even though
 * length here is an unsigned int; this avoids problems
 * if the length is passed through an interface with type "int",
 * and is absurdly large anyway.
 *
 * Earlier versions of the source code incorrectly stated that
 * examining the text string alone can be unconditionally valid. This
 * is incorrect, as some token types strip parts of the string,
 * e.g. indirect tokens.
 */
#define INLINE_TEXT (7*sizeof(char *)-sizeof(enum token_type)-sizeof(unsigned int)-1)
#define MAX_TEXT (INT_MAX-2)

struct Token {
    Token *next;
    enum token_type type;
    unsigned int len;
    union {
        char a[INLINE_TEXT+1];
        struct {
            char pad[INLINE_TEXT+1 - sizeof(char *)];
            char *ptr;
        } p;
    } text;
};

/*
 * Note on the storage of both SMacro and MMacros: the hash table
 * indexes them case-insensitively, and we then have to go through a
 * linked list of potential case aliases (and, for MMacros, parameter
 * ranges); this is to preserve the matching semantics of the earlier
 * code.  If the number of case aliases for a specific macro is a
 * performance issue, you may want to reconsider your coding style.
 */

/*
 * Function call tp obtain the expansion of an smacro
 */
typedef Token *(*ExpandSMacro)(const SMacro *s, Token **params, int nparams);

/*
 * Store the definition of a single-line macro.
 *
 * Note: for user-defined macros, SPARM_VARADIC and SPARM_DEFAULT are
 * currently never set, and SPARM_OPTIONAL is set if and only
 * if SPARM_GREEDY is set.
 */
enum sparmflags {
    SPARM_PLAIN     =   0,
    SPARM_EVAL      =   1,  /* Evaluate as a numeric expression (=) */
    SPARM_STR       =   2,  /* Convert to quoted string ($) */
    SPARM_NOSTRIP   =   4,  /* Don't strip braces (!) */
    SPARM_GREEDY    =   8,  /* Greedy final parameter (+) */
    SPARM_VARADIC   =  16,  /* Any number of separate arguments */
    SPARM_OPTIONAL  =  32,  /* Optional argument */
    SPARM_CONDQUOTE =  64,  /* With SPARM_STR, don't re-quote a string */
    SPARM_UNSIGNED  = 128   /* With SPARM_EVAL, generate unsigned numbers */
};

struct smac_param {
    Token name;
    enum sparmflags flags;
    char radix;                 /* Radix type for SPARM_EVAL */
    const Token *def;           /* Default, if any */
};

struct SMacro {
    SMacro *next;               /* MUST BE FIRST - see free_smacro() */
    char *name;
    Token *expansion;
    ExpandSMacro expand;
    intorptr expandpvt;
    struct smac_param *params;
    int nparam;                 /* length of the params structure */
    int nparam_min;             /* allows < nparam arguments */
    int in_progress;
    bool recursive;
    bool varadic;               /* greedy or supports > nparam arguments */
    bool casesense;
    bool alias;                 /* This is an alias macro */
};

/*
 * "No listing" flags. Inside a loop (%rep..%endrep) we may have
 * macro listing suppressed with .nolist, but we still need to
 * update line numbers for error messages and debug information...
 * unless we are nested inside an actual .nolist macro.
 */
enum nolist_flags {
    NL_LIST   = 1,              /* Suppress list output */
    NL_LINE   = 2               /* Don't update line information */
};

/*
 * Store the definition of a multi-line macro. This is also used to
 * store the interiors of `%rep...%endrep' blocks, which are
 * effectively self-re-invoking multi-line macros which simply
 * don't have a name or bother to appear in the hash tables. %rep
 * blocks are signified by having a NULL `name' field.
 *
 * In a MMacro describing a `%rep' block, the `in_progress' field
 * isn't merely boolean, but gives the number of repeats left to
 * run.
 *
 * The `next' field is used for storing MMacros in hash tables; the
 * `next_active' field is for stacking them on istk entries.
 *
 * When a MMacro is being expanded, `params', `iline', `nparam',
 * `paramlen', `rotate' and `unique' are local to the invocation.
 */

/*
 * Expansion stack. Note that .mmac can point back to the macro itself,
 * whereas .mstk cannot.
 */
struct mstk {
    MMacro *mstk;               /* Any expansion, real macro or not */
    MMacro *mmac;               /* Highest level actual mmacro */
};

struct MMacro {
    MMacro *next;
#if 0
    MMacroInvocation *prev;     /* previous invocation */
#endif
    char *name;
    int nparam_min, nparam_max;
    enum nolist_flags nolist;   /* is this macro listing-inhibited? */
    bool casesense;
    bool plus;                  /* is the last parameter greedy? */
    bool capture_label;         /* macro definition has %00; capture label */
    int32_t in_progress;        /* is this macro currently being expanded? */
    int32_t max_depth;          /* maximum number of recursive expansions allowed */
    Token *dlist;               /* All defaults as one list */
    Token **defaults;           /* Parameter default pointers */
    int ndefs;                  /* number of default parameters */
    Line *expansion;

    struct mstk mstk;           /* Macro expansion stack */
    struct mstk dstk;           /* Macro definitions stack */
    Token **params;             /* actual parameters */
    Token *iline;               /* invocation line */
    struct src_location where;  /* location of definition */
    unsigned int nparam, rotate;
    char *iname;                /* name invoked as */
    int *paramlen;
    uint64_t unique;
    uint64_t condcnt;           /* number of if blocks... */
    struct {                    /* Debug information */
        struct debug_macro_def *def; /* Definition */
        struct debug_macro_inv *inv; /* Current invocation (if any) */
    } dbg;
};


/* Store the definition of a multi-line macro, as defined in a
 * previous recursive macro expansion.
 */
#if 0

struct MMacroInvocation {
    MMacroInvocation *prev;     /* previous invocation */
    Token **params;             /* actual parameters */
    Token *iline;               /* invocation line */
    unsigned int nparam, rotate;
    int *paramlen;
    uint64_t unique;
    uint64_t condcnt;
};

#endif

/*
 * The context stack is composed of a linked list of these.
 */
struct Context {
    Context *next;
    const char *name;
    struct hash_table localmac;
    uint64_t number;
    unsigned int depth;
};


static inline const char *tok_text(const struct Token *t)
{
    return (t->len <= INLINE_TEXT) ? t->text.a : t->text.p.ptr;
}

/*
 * Returns a mutable pointer to the text buffer. The text can be changed,
 * but the length MUST NOT CHANGE, in either direction; nor is it permitted
 * to pad with null characters to create an artificially shorter string.
 */
static inline char *tok_text_buf(struct Token *t)
{
    return (t->len <= INLINE_TEXT) ? t->text.a : t->text.p.ptr;
}

static inline unsigned int tok_check_len(size_t len)
{
    if (unlikely(len > MAX_TEXT))
	nasm_fatal("impossibly large token");

    return len;
}

static inline bool tok_text_match(const struct Token *a, const struct Token *b)
{
    return a->len == b->len && !memcmp(tok_text(a), tok_text(b), a->len);
}

static inline unused_func bool
tok_match(const struct Token *a, const struct Token *b)
{
    return a->type == b->type && tok_text_match(a, b);
}

/* strlen() variant useful for set_text() and its variants */
static size_t tok_strlen(const char *str)
{
    return strnlen(str, MAX_TEXT+1);
}

/*
 * Set the text field to a copy of the given string; the length if
 * not given should be obtained with tok_strlen().
 */
static Token *set_text(struct Token *t, const char *text, size_t len)
{
    char *textp;

    if (t->len > INLINE_TEXT)
	nasm_free(t->text.p.ptr);

    nasm_zero(t->text);

    t->len = len = tok_check_len(len);
    textp = (len > INLINE_TEXT)
	? (t->text.p.ptr = nasm_malloc(len+1)) : t->text.a;
    memcpy(textp, text, len);
    textp[len] = '\0';
    return t;
}

/*
 * Set the text field to the existing pre-allocated string, either
 * taking over or freeing the allocation in the process.
 */
static Token *set_text_free(struct Token *t, char *text, unsigned int len)
{
    char *textp;

    if (t->len > INLINE_TEXT)
	nasm_free(t->text.p.ptr);

    nasm_zero(t->text);

    t->len = len = tok_check_len(len);
    if (len > INLINE_TEXT) {
	textp = t->text.p.ptr = text;
    } else {
	textp = memcpy(t->text.a, text, len);
	nasm_free(text);
    }
    textp[len] = '\0';

    return t;
}

/*
 * Allocate a new buffer containing a copy of the text field
 * of the token.
 */
static char *dup_text(const struct Token *t)
{
    size_t size = t->len + 1;
    char *p = nasm_malloc(size);

    return memcpy(p, tok_text(t), size);
}

/*
 * Multi-line macro definitions are stored as a linked list of
 * these, which is essentially a container to allow several linked
 * lists of Tokens.
 *
 * Note that in this module, linked lists are treated as stacks
 * wherever possible. For this reason, Lines are _pushed_ on to the
 * `expansion' field in MMacro structures, so that the linked list,
 * if walked, would give the macro lines in reverse order; this
 * means that we can walk the list when expanding a macro, and thus
 * push the lines on to the `expansion' field in _istk_ in reverse
 * order (so that when popped back off they are in the right
 * order). It may seem cockeyed, and it relies on my design having
 * an even number of steps in, but it works...
 *
 * Some of these structures, rather than being actual lines, are
 * markers delimiting the end of the expansion of a given macro.
 * This is for use in the cycle-tracking and %rep-handling code.
 * Such structures have `finishes' non-NULL, and `first' NULL. All
 * others have `finishes' NULL, but `first' may still be NULL if
 * the line is blank.
 */
struct Line {
    Line *next;
    MMacro *finishes;
    Token *first;
    struct src_location where;      /* Where defined */
};

/*
 * To handle an arbitrary level of file inclusion, we maintain a
 * stack (ie linked list) of these things.
 *
 * Note: when we issue a message for a continuation line, we want to
 * issue it for the actual *start* of the continuation line. This means
 * we need to remember how many lines to skip over for the next one.
 */
struct Include {
    Include *next;
    FILE *fp;
    Cond *conds;
    Line *expansion;
    uint64_t nolist;            /* Listing inhibit counter */
    uint64_t noline;            /* Line number update inhibit counter */
    struct mstk mstk;
    struct src_location where;  /* Filename and current line number */
    int32_t lineinc;            /* Increment given by %line */
    int32_t lineskip;           /* Accounting for passed continuation lines */
};

/*
 * File real name hash, so we don't have to re-search the include
 * path for every pass (and potentially more than that if a file
 * is used more than once.)
 */
struct hash_table FileHash;

/*
 * Counters to trap on insane macro recursion or processing.
 * Note: for smacros these count *down*, for mmacros they count *up*.
 */
struct deadman {
    int64_t total;              /* Total number of macros/tokens */
    int64_t levels;             /* Descent depth across all macros */
    bool triggered;             /* Already triggered, no need for error msg */
};

static struct deadman smacro_deadman, mmacro_deadman;

/*
 * Conditional assembly: we maintain a separate stack of these for
 * each level of file inclusion. (The only reason we keep the
 * stacks separate is to ensure that a stray `%endif' in a file
 * included from within the true branch of a `%if' won't terminate
 * it and cause confusion: instead, rightly, it'll cause an error.)
 */
enum cond_state {
    /*
     * These states are for use just after %if or %elif: IF_TRUE
     * means the condition has evaluated to truth so we are
     * currently emitting, whereas IF_FALSE means we are not
     * currently emitting but will start doing so if a %else comes
     * up. In these states, all directives are admissible: %elif,
     * %else and %endif. (And of course %if.)
     */
    COND_IF_TRUE, COND_IF_FALSE,
    /*
     * These states come up after a %else: ELSE_TRUE means we're
     * emitting, and ELSE_FALSE means we're not. In ELSE_* states,
     * any %elif or %else will cause an error.
     */
    COND_ELSE_TRUE, COND_ELSE_FALSE,
    /*
     * These states mean that we're not emitting now, and also that
     * nothing until %endif will be emitted at all. COND_DONE is
     * used when we've had our moment of emission
     * and have now started seeing %elifs. COND_NEVER is used when
     * the condition construct in question is contained within a
     * non-emitting branch of a larger condition construct,
     * or if there is an error.
     */
    COND_DONE, COND_NEVER
};
struct Cond {
    Cond *next;
    enum cond_state state;
};
#define emitting(x) ( (x) == COND_IF_TRUE || (x) == COND_ELSE_TRUE )

/*
 * These defines are used as the possible return values for do_directive
 */
#define NO_DIRECTIVE_FOUND  0
#define DIRECTIVE_FOUND     1

/*
 * Condition codes. Note that we use c_ prefix not C_ because C_ is
 * used in nasm.h for the "real" condition codes. At _this_ level,
 * we treat CXZ and ECXZ as condition codes, albeit non-invertible
 * ones, so we need a different enum...
 */
static const char * const conditions[] = {
    "a", "ae", "b", "be", "c", "cxz", "e", "ecxz", "g", "ge", "l", "le",
    "na", "nae", "nb", "nbe", "nc", "ne", "ng", "nge", "nl", "nle", "no",
    "np", "ns", "nz", "o", "p", "pe", "po", "rcxz", "s", "z"
};
enum pp_conds {
    c_A, c_AE, c_B, c_BE, c_C, c_CXZ, c_E, c_ECXZ, c_G, c_GE, c_L, c_LE,
    c_NA, c_NAE, c_NB, c_NBE, c_NC, c_NE, c_NG, c_NGE, c_NL, c_NLE, c_NO,
    c_NP, c_NS, c_NZ, c_O, c_P, c_PE, c_PO, c_RCXZ, c_S, c_Z,
    c_none = -1
};
static const enum pp_conds inverse_ccs[] = {
    c_NA, c_NAE, c_NB, c_NBE, c_NC, -1, c_NE, -1, c_NG, c_NGE, c_NL, c_NLE,
    c_A, c_AE, c_B, c_BE, c_C, c_E, c_G, c_GE, c_L, c_LE, c_O, c_P, c_S,
    c_Z, c_NO, c_NP, c_PO, c_PE, -1, c_NS, c_NZ
};

/*
 * Directive names.
 */
/* If this is a an IF, ELIF, ELSE or ENDIF keyword */
static int is_condition(enum preproc_token arg)
{
    return PP_IS_COND(arg) || (arg == PP_ELSE) || (arg == PP_ENDIF);
}

static int StackSize = 4;
static const char *StackPointer = "ebp";
static int ArgOffset = 8;
static int LocalOffset = 0;

static Context *cstk;
static Include *istk;
static const struct strlist *ipath_list;

static struct strlist *deplist;

static uint64_t unique;     /* unique identifier numbers */

static Line *predef = NULL;
static bool do_predef;
static enum preproc_mode pp_mode;

/*
 * The current set of multi-line macros we have defined.
 */
static struct hash_table mmacros;

/*
 * The current set of single-line macros we have defined.
 */
static struct hash_table smacros;

/*
 * The multi-line macro we are currently defining, or the %rep
 * block we are currently reading, if any.
 */
static MMacro *defining;

static uint64_t nested_mac_count;
static uint64_t nested_rep_count;

/*
 * The number of macro parameters to allocate space for at a time.
 */
#define PARAM_DELTA 16

/*
 * The standard macro set: defined in macros.c in a set of arrays.
 * This gives our position in any macro set, while we are processing it.
 * The stdmacset is an array of such macro sets.
 */
static macros_t *stdmacpos;
static macros_t **stdmacnext;
static macros_t *stdmacros[8];
static macros_t *extrastdmac;

/*
 * Map of which %use packages have been loaded
 */
static bool *use_loaded;

/*
 * Forward declarations.
 */
static void pp_add_stdmac(macros_t *macros);
static Token *expand_mmac_params(Token * tline);
static Token *expand_smacro(Token * tline);
static Token *expand_id(Token * tline);
static Context *get_ctx(const char *name, const char **namep);
static Token *make_tok_num(Token *next, int64_t val);
static Token *
make_tok_num_radix(Token *next, int64_t val, char radix, bool uns);
static int64_t get_tok_num(const Token *t, bool *err);
static Token *make_tok_qstr(Token *next, const char *str);
static Token *make_tok_qstr_len(Token *next, const char *str, size_t len);
static Token *make_tok_char(Token *next, char op);
static Token *new_Token(Token * next, enum token_type type,
                        const char *text, size_t txtlen);
static Token *new_Token_free(Token * next, enum token_type type,
                             char *text, size_t txtlen);
static Token *dup_Token(Token *next, const Token *src);
static Token *new_White(Token *next);
static Token *delete_Token(Token *t);
static Token *steal_Token(Token *dst, Token *src);
static const struct use_package *
get_use_pkg(Token *t, const char *dname, const char **name);
static void mark_smac_params(Token *tline, const SMacro *tmpl,
                             enum token_type type);

/* Safe extraction of token type */
static inline enum token_type tok_type(const Token *x)
{
    return x ? x->type : TOKEN_EOS;
}

/* Safe test for token type, false on x == NULL */
static inline bool tok_is(const Token *x, enum token_type t)
{
    return tok_type(x) == t;
}
/* True if token is any other kind other that "c", but not NULL */
static inline bool tok_isnt(const Token *x, enum token_type t)
{
    return x && x->type != t;
}

/* Whitespace token? */
static inline bool tok_white(const Token *x)
{
    return tok_is(x, TOKEN_WHITESPACE);
}

/* Skip past any whitespace */
static inline Token *skip_white(Token *x)
{
    while (tok_white(x))
        x = x->next;

    return x;
}

/* Delete any whitespace */
static Token *zap_white(Token *x)
{
    while (tok_white(x))
        x = delete_Token(x);

    return x;
}

/*
 * Unquote a token if it is a string, and set its type to
 * TOKEN_INTERNAL_STR.
 */

/*
 * Common version for any kind of quoted string; see asm/quote.c for
 * information about the arguments.
 */
static const char *unquote_token_anystr(Token *t, uint32_t badctl, char qstart)
{
    size_t nlen, olen;
    char *p;

    if (t->type != TOKEN_STR)
	return tok_text(t);

    olen = t->len;
    p = (olen > INLINE_TEXT) ? t->text.p.ptr : t->text.a;
    t->len = nlen = nasm_unquote_anystr(p, NULL, badctl, qstart);
    t->type = TOKEN_INTERNAL_STR;

    if (olen <= INLINE_TEXT || nlen > INLINE_TEXT)
        return p;

    nasm_zero(t->text.a);
    memcpy(t->text.a, p, nlen);
    nasm_free(p);
    return t->text.a;
}

/* Unquote any string, can produce any arbitrary binary output */
static const char *unquote_token(Token *t)
{
    return unquote_token_anystr(t, 0, STR_NASM);
}

/*
 * Same as unquote_token(), but error out if the resulting string
 * contains unacceptable control characters.
 */
static const char *unquote_token_cstr(Token *t)
{
    return unquote_token_anystr(t, BADCTL, STR_NASM);
}

/*
 * Convert a TOKEN_INTERNAL_STR token to a quoted
 * TOKEN_STR tokens.
 */
static Token *quote_any_token(Token *t);
static inline unused_func
Token *quote_token(Token *t)
{
    if (likely(!tok_is(t, TOKEN_INTERNAL_STR)))
	return t;

    return quote_any_token(t);
}

/*
 * Convert *any* kind of token to a quoted
 * TOKEN_STR token.
 */
static Token *quote_any_token(Token *t)
{
    size_t len = t->len;
    char *p;

    p = nasm_quote(tok_text(t), &len);
    t->type = TOKEN_STR;
    return set_text_free(t, p, len);
}

/*
 * In-place reverse a list of tokens.
 */
static Token *reverse_tokens(Token *t)
{
    Token *prev = NULL;
    Token *next;

    while (t) {
        next = t->next;
        t->next = prev;
        prev = t;
        t = next;
    }

    return prev;
}

/*
 * getenv() variant operating on an input token
 */
static const char *pp_getenv(const Token *t, bool warn)
{
    const char *txt = tok_text(t);
    const char *v;
    char *buf = NULL;
    bool is_string = false;

    if (!t)
	return NULL;

    switch (t->type) {
    case TOKEN_ENVIRON:
	txt += 2;		/* Skip leading %! */
	is_string = nasm_isquote(*txt);
	break;

    case TOKEN_STR:
	is_string = true;
	break;

    case TOKEN_INTERNAL_STR:
    case TOKEN_NAKED_STR:
    case TOKEN_ID:
	is_string = false;
	break;

    default:
	return NULL;
    }

    if (is_string) {
	buf = nasm_strdup(txt);
	nasm_unquote_cstr(buf, NULL);
	txt = buf;
    }

    v = getenv(txt);
    if (warn && !v) {
	/*!
	 *!pp-environment [on] nonexistent environment variable
         *!=environment
	 *!  warns if a nonexistent environment variable
	 *!  is accessed using the \c{%!} preprocessor
	 *!  construct (see \k{getenv}.)  Such environment
	 *!  variables are treated as empty (with this
	 *!  warning issued) starting in NASM 2.15;
	 *!  earlier versions of NASM would treat this as
	 *!  an error.
	 */
	nasm_warn(WARN_PP_ENVIRONMENT,
                  "nonexistent environment variable `%s'", txt);
	v = "";
    }

    if (buf)
	nasm_free(buf);

    return v;
}

/*
 * Free a linked list of tokens.
 */
static void free_tlist(Token * list)
{
    while (list)
        list = delete_Token(list);
}

/*
 * Free a linked list of lines.
 */
static void free_llist(Line * list)
{
    Line *l, *tmp;
    list_for_each_safe(l, tmp, list) {
        free_tlist(l->first);
        nasm_free(l);
    }
}

/*
 * Free an array of linked lists of tokens
 */
static void free_tlist_array(Token **array, size_t nlists)
{
    Token **listp = array;

    if (!array)
        return;

    while (nlists--)
        free_tlist(*listp++);

    nasm_free(array);
}

/*
 * Duplicate a linked list of tokens.
 */
static Token *dup_tlist(const Token *list, Token ***tailp)
{
    Token *newlist = NULL;
    Token **tailpp = &newlist;
    const Token *t;

    list_for_each(t, list) {
        Token *nt;
        *tailpp = nt = dup_Token(NULL, t);
        tailpp = &nt->next;
    }

    if (tailp) {
        **tailp = newlist;
        *tailp = tailpp;
    }

    return newlist;
}

/*
 * Duplicate a linked list of tokens with a maximum count
 */
static Token *dup_tlistn(const Token *list, size_t cnt, Token ***tailp)
{
    Token *newlist = NULL;
    Token **tailpp = &newlist;
    const Token *t;

    list_for_each(t, list) {
        Token *nt;
        if (!cnt--)
            break;
        *tailpp = nt = dup_Token(NULL, t);
        tailpp = &nt->next;
    }

    if (tailp) {
        **tailp = newlist;
        if (newlist)
            *tailp = tailpp;
    }

    return newlist;
}

/*
 * Duplicate a linked list of tokens in reverse order
 */
static Token *dup_tlist_reverse(const Token *list, Token *tail)
{
    const Token *t;

    list_for_each(t, list)
        tail = dup_Token(tail, t);

    return tail;
}

/*
 * Append an existing tlist to a tail pointer and returns the
 * updated tail pointer.
 */
static Token **steal_tlist(Token *tlist, Token **tailp)
{
    *tailp = tlist;

    if (!tlist)
        return tailp;

    list_last(tlist, tlist);
    return &tlist->next;
}

/*
 * Free an MMacro
 */
static void free_mmacro(MMacro * m)
{
    nasm_free(m->name);
    free_tlist(m->dlist);
    nasm_free(m->defaults);
    free_llist(m->expansion);
    nasm_free(m);
}

/*
 * Clear or free an SMacro
 */
static void free_smacro_members(SMacro *s)
{
    if (s->params) {
        int i;
        for (i = 0; i < s->nparam; i++) {
	    if (s->params[i].name.len > INLINE_TEXT)
		nasm_free(s->params[i].name.text.p.ptr);
            if (s->params[i].def)
                free_tlist((Token *)s->params[i].def);
	}
        nasm_free(s->params);
    }
    nasm_free(s->name);
    free_tlist(s->expansion);
}

static void clear_smacro(SMacro *s)
{
    free_smacro_members(s);
    /* Wipe everything except the next pointer */
    memset(&s->name, 0, sizeof(*s) - offsetof(SMacro, name));
}

/*
 * Free an SMacro
 */
static void free_smacro(SMacro *s)
{
    free_smacro_members(s);
    nasm_free(s);
}

/*
 * Free all currently defined macros, and free the hash tables if empty
 */
enum clear_what {
    CLEAR_NONE      = 0,
    CLEAR_DEFINE    = 1,         /* Clear smacros */
    CLEAR_DEFALIAS  = 2,         /* Clear smacro aliases */
    CLEAR_ALLDEFINE = CLEAR_DEFINE|CLEAR_DEFALIAS,
    CLEAR_MMACRO    = 4,
    CLEAR_ALL       = CLEAR_ALLDEFINE|CLEAR_MMACRO
};

static void clear_smacro_table(struct hash_table *smt, enum clear_what what)
{
    struct hash_iterator it;
    const struct hash_node *np;
    bool empty = true;

    /*
     * Walk the hash table and clear out anything we don't want
     */
    hash_for_each(smt, it, np) {
        SMacro *tmp;
        SMacro *s = np->data;
        SMacro **head = (SMacro **)&np->data;

        list_for_each_safe(s, tmp, s) {
            if (what & ((enum clear_what)s->alias + 1)) {
                *head = s->next;
                free_smacro(s);
            } else {
                empty = false;
            }
        }
    }

    /*
     * Free the hash table and keys if and only if it is now empty.
     * Note: we cannot free keys even for an empty list above, as that
     * mucks up the hash algorithm.
     */
    if (empty)
        hash_free_all(smt, true);
}

static void free_smacro_table(struct hash_table *smt)
{
    clear_smacro_table(smt, CLEAR_ALLDEFINE);
}

static void free_mmacro_table(struct hash_table *mmt)
{
    struct hash_iterator it;
    const struct hash_node *np;

    hash_for_each(mmt, it, np) {
        MMacro *tmp;
        MMacro *m = np->data;
        nasm_free((void *)np->key);
        list_for_each_safe(m, tmp, m)
            free_mmacro(m);
    }
    hash_free(mmt);
}

static void free_macros(void)
{
    free_smacro_table(&smacros);
    free_mmacro_table(&mmacros);
}

/*
 * Initialize the hash tables
 */
static void init_macros(void)
{
}

/*
 * Pop the context stack.
 */
static void ctx_pop(void)
{
    Context *c = cstk;

    cstk = cstk->next;
    free_smacro_table(&c->localmac);
    nasm_free((char *)c->name);
    nasm_free(c);
}

/*
 * Search for a key in the hash index; adding it if necessary
 * (in which case we initialize the data pointer to NULL.)
 */
static void **
hash_findi_add(struct hash_table *hash, const char *str)
{
    struct hash_insert hi;
    void **r;
    char *strx;
    size_t l = strlen(str) + 1;

    r = hash_findib(hash, str, l, &hi);
    if (r)
        return r;

    strx = nasm_malloc(l);  /* Use a more efficient allocator here? */
    memcpy(strx, str, l);
    return hash_add(&hi, strx, NULL);
}

/*
 * Like hash_findi, but returns the data element rather than a pointer
 * to it.  Used only when not adding a new element, hence no third
 * argument.
 */
static void *
hash_findix(struct hash_table *hash, const char *str)
{
    void **p;

    p = hash_findi(hash, str, NULL);
    return p ? *p : NULL;
}

/*
 * read line from standard macros set,
 * if there no more left -- return NULL
 */
static char *line_from_stdmac(void)
{
    unsigned char c;
    const unsigned char *p = stdmacpos;
    char *line, *q;
    size_t len = 0;

    if (!stdmacpos)
        return NULL;

    /*
     * 32-126 is ASCII, 127 is end of line, 128-31 are directives
     * (allowed to wrap around) corresponding to PP_* tokens 0-159.
     */
    while ((c = *p++) != 127) {
        uint8_t ndir = c - 128;
        if (ndir < 256-96)
            len += pp_directives_len[ndir] + 1;
        else
            len++;
    }

    line = nasm_malloc(len + 1);
    q = line;

    while ((c = *stdmacpos++) != 127) {
        uint8_t ndir = c - 128;
        if (ndir < 256-96) {
            memcpy(q, pp_directives[ndir], pp_directives_len[ndir]);
            q += pp_directives_len[ndir];
            *q++ = ' ';
        } else {
            *q++ = c;
        }
    }
    stdmacpos = p;
    *q = '\0';

    if (*stdmacpos == 127) {
        /* This was the last of this particular macro set */
        stdmacpos = NULL;
        if (*stdmacnext) {
            stdmacpos = *stdmacnext++;
        } else if (do_predef) {
            Line *pd, *l;

            /*
             * Nasty hack: here we push the contents of
             * `predef' on to the top-level expansion stack,
             * since this is the most convenient way to
             * implement the pre-include and pre-define
             * features.
             */
            list_for_each(pd, predef) {
                nasm_new(l);
                l->next     = istk->expansion;
                l->first    = dup_tlist(pd->first, NULL);
                l->finishes = NULL;

                istk->expansion = l;
            }
            do_predef = false;
        }
    }

    return line;
}

/*
 * Read a line from a file. Return NULL on end of file.
 */
static char *line_from_file(FILE *f)
{
    int c;
    unsigned int size, next;
    const unsigned int delta = 512;
    const unsigned int pad = 8;
    bool cont = false;
    char *buffer, *p;

    istk->where.lineno += istk->lineskip + istk->lineinc;
    src_set_linnum(istk->where.lineno);
    istk->lineskip = 0;

    size = delta;
    p = buffer = nasm_malloc(size);

    do {
        c = fgetc(f);

        switch (c) {
        case EOF:
            if (p == buffer) {
                nasm_free(buffer);
                return NULL;
            }
            c = 0;
            break;

        case '\r':
            next = fgetc(f);
            if (next != '\n')
                ungetc(next, f);
            if (cont) {
                cont = false;
                continue;
            }
            c = 0;
            break;

        case '\n':
            if (cont) {
                cont = false;
                continue;
            }
            c = 0;
            break;

        case 032:               /* ^Z = legacy MS-DOS end of file mark */
            c = 0;
            break;

        case '\\':
            next = fgetc(f);
            ungetc(next, f);
            if (next == '\r' || next == '\n') {
                cont = true;
                istk->lineskip += istk->lineinc;
                continue;
            }
            break;
        }

        if (p >= (buffer + size - pad)) {
            buffer = nasm_realloc(buffer, size + delta);
            p = buffer + size - pad;
            size += delta;
        }

        *p++ = c;
    } while (c);

    return buffer;
}

/*
 * Common read routine regardless of source
 */
static char *read_line(void)
{
    char *line;
    FILE *f = istk->fp;

    if (f)
        line = line_from_file(f);
    else
        line = line_from_stdmac();

    if (!line)
        return NULL;

    if (!istk->nolist)
        lfmt->line(LIST_READ, istk->where.lineno, line);

    return line;
}

/*
 * Tokenize a line of text. This is a very simple process since we
 * don't need to parse the value out of e.g. numeric tokens: we
 * simply split one string into many.
 */
static Token *tokenize(const char *line)
{
    enum token_type type;
    Token *list = NULL;
    Token *t, **tail = &list;

    while (*line) {
        const char *p = line;
        const char *ep = NULL;  /* End of token, for trimming the end */
        size_t toklen;
        char firstchar = *p;    /* Can be used to override the first char */

        if (*p == '%') {
            /*
             * Preprocessor construct; find the end of the token.
             * Classification is handled later, because %{...} can be
             * used to create any preprocessor token.
             */
            p++;
            if (*p == '+' && !nasm_isdigit(p[1])) {
                /* Paste token */
                p++;
            } else if (nasm_isdigit(*p) ||
                       ((*p == '-' || *p == '+') && nasm_isdigit(p[1]))) {
                do {
                    p++;
                }
                while (nasm_isdigit(*p));
            } else if (*p == '{' || *p == '[') {
                /* %{...} or %[...] */
                char firstchar = *p;
                char endchar = *p + 2; /* } or ] */
                int lvl = 1;
                line += (*p++ == '{'); /* Skip { but not [ (yet) */
                while (lvl) {
                    if (*p == firstchar) {
                        lvl++;
                    } else if (*p == endchar) {
                        lvl--;
                    } else if (nasm_isquote(*p)) {
                        p = nasm_skip_string(p);
                    }

                    /*
                     * *p can have been advanced to a null character by
                     * nasm_skip_string()
                     */
                    if (!*p) {
                        /*!
                         *!pp-open-brackets [on] unterminated \c{%[...]}
                         *!  warns that a preprocessor \c{%[...]} construct
                         *!  lacks the terminating \c{]} character.
                         */
                        /*!
                         *!pp-open-braces [on] unterminated \c{%\{...\}}
                         *!  warns that a preprocessor parameter
                         *!  enclosed in braces \c{%\{...\}} lacks the
                         *!  terminating \c{\}} character.
                         */
                        nasm_warn(firstchar == '}' ?
                                  WARN_PP_OPEN_BRACES : WARN_PP_OPEN_BRACKETS,
                                  "unterminated %%%c...%c construct (missing `%c')",
                                  firstchar, endchar, endchar);
                        break;
                    }
                    p++;
                }
                ep = lvl ? p : p-1; /* Terminal character not part of token */
            } else if (*p == '?') {
                /* %? or %?? */
                p++;
                if (*p == '?')
                    p++;
            } else if (*p == '*' && p[1] == '?') {
                /* %*? and %*?? */
                p += 2;
                if (*p == '?')
                    p++;
            } else if (*p == '!') {
                /* Environment variable reference */
                p++;
                if (nasm_isidchar(*p)) {
                    do {
                        p++;
                    }
                    while (nasm_isidchar(*p));
                } else if (nasm_isquote(*p)) {
                    p = nasm_skip_string(p);
                    if (*p)
                        p++;
                    else
                        nasm_nonfatal("unterminated %%! string");
                } else {
                    /* %! without anything else... */
                }
            } else if (*p == ',') {
                /* Conditional comma */
                p++;
            } else if (nasm_isidchar(*p) ||
                       ((*p == '%' || *p == '$') && nasm_isidchar(p[1]))) {
                /* Identifier or some sort */
                do {
                    p++;
                }
                while (nasm_isidchar(*p));
            } else if (*p == '%') {
                /* %% operator */
                p++;
            }

            if (!ep)
                ep = p;
            toklen = ep - line;

            /* Classify here, to handle %{...} correctly */
            if (toklen < 2) {
                type = '%';     /* % operator */
                if (unlikely(*line == '{')) {
                    /*!
                     *!pp-empty-braces [on] empty \c{%\{\}} construct
                     *!  warns that an empty \c{%\{\}} was encountered.
                     *!  This expands to a single \c{%} character, which
                     *!  is normally the \c{%} arithmetic operator.
                     */
                    nasm_warn(WARN_PP_EMPTY_BRACES,
                              "empty %%{} construct expands to the %% operator");
                }
            } else {
                char c0 = line[1];

                switch (c0) {
                case '+':
                    type = (toklen == 2) ? TOKEN_PASTE : TOKEN_MMACRO_PARAM;
                    break;

                case '-':
                    type = TOKEN_MMACRO_PARAM;
                    break;

                case '?':
                    if (toklen == 2)
                        type = TOKEN_PREPROC_Q;
                    else if (toklen == 3 && line[2] == '?')
                        type = TOKEN_PREPROC_QQ;
                    else
                        type = TOKEN_PREPROC_ID;
                    break;

                case '*':
                    type = TOKEN_OTHER;
                    if (line[2] == '?') {
                        if (toklen == 3)
                            type = TOKEN_PREPROC_SQ;
                        else if (toklen == 4 && line[3] == '?')
                            type = TOKEN_PREPROC_SQQ;
                    }
                    break;

                case '!':
                    type = (toklen == 2) ? TOKEN_OTHER : TOKEN_ENVIRON;
                    break;

                case '%':
                    type = (toklen == 2) ? TOKEN_SMOD : TOKEN_LOCAL_SYMBOL;
                    break;

                case '$':
                    type = (toklen == 2) ? TOKEN_OTHER : TOKEN_LOCAL_MACRO;
                    break;

                case '[':
                    line += 2;  /* Skip %[ */
                    firstchar = *line; /* Don't clobber */
                    toklen -= 2;
                    type = TOKEN_INDIRECT;
                    break;

                case ',':
                    type = (toklen == 2) ? TOKEN_COND_COMMA : TOKEN_PREPROC_ID;
                    break;

                case '\'':
                case '\"':
                case '`':
                    /* %{'string'} */
                    type = TOKEN_PREPROC_ID;
                    break;

                case ':':
                    type = TOKEN_MMACRO_PARAM; /* %{:..} */
                    break;

                default:
                    if (nasm_isdigit(c0))
                        type = TOKEN_MMACRO_PARAM;
                    else if (nasm_isidchar(c0) || toklen > 2)
                        type = TOKEN_PREPROC_ID;
                    else
                        type = TOKEN_OTHER;
                    break;
                }
            }
        } else if (*p == '?' && !nasm_isidchar(p[1])) {
            /* ? operator */
            type = TOKEN_QMARK;
            p++;
        } else if (nasm_isidstart(*p) || (*p == '$' && nasm_isidstart(p[1]))) {
            /*
             * A regular identifier. This includes keywords which are not
             * special to the preprocessor.
             */
            type = TOKEN_ID;
            while (nasm_isidchar(*++p))
                ;
         } else if (nasm_isquote(*p)) {
            /*
             * A string token.
             */
            char quote = *p;

            type = TOKEN_STR;
            p = nasm_skip_string(p);

            if (*p) {
                p++;
            } else {
                /*!
                 *!pp-open-string [on] unterminated string
                 *!  warns that a quoted string without a closing quotation
                 *!  mark was encountered during preprocessing.
                 */
                nasm_warn(WARN_PP_OPEN_STRING,
                          "unterminated string (missing `%c')", quote);
                type = TOKEN_ERRSTR;
            }
        } else if (p[0] == '$' && p[1] == '$') {
            type = TOKEN_BASE;
            p += 2;
        } else if (nasm_isnumstart(*p)) {
            bool is_hex = false;
            bool is_float = false;
            bool has_e = false;
            char c;

            /*
             * A numeric token.
             */

            if (*p == '$') {
                p++;
                is_hex = true;
            }

            for (;;) {
                c = *p++;

                if (!is_hex && (c == 'e' || c == 'E')) {
                    has_e = true;
                    if (*p == '+' || *p == '-') {
                        /*
                         * e can only be followed by +/- if it is either a
                         * prefixed hex number or a floating-point number
                         */
                        p++;
                        is_float = true;
                    }
                } else if (c == 'H' || c == 'h' || c == 'X' || c == 'x') {
                    is_hex = true;
                } else if (c == 'P' || c == 'p') {
                    is_float = true;
                    if (*p == '+' || *p == '-')
                        p++;
                } else if (nasm_isnumchar(c))
                    ; /* just advance */
                else if (c == '.') {
                    /*
                     * we need to deal with consequences of the legacy
                     * parser, like "1.nolist" being two tokens
                     * (TOKEN_NUM, TOKEN_ID) here; at least give it
                     * a shot for now.  In the future, we probably need
                     * a flex-based scanner with proper pattern matching
                     * to do it as well as it can be done.  Nothing in
                     * the world is going to help the person who wants
                     * 0x123.p16 interpreted as two tokens, though.
                     */
                    const char *r = p;
                    while (*r == '_')
                        r++;

                    if (nasm_isdigit(*r) || (is_hex && nasm_isxdigit(*r)) ||
                        (!is_hex && (*r == 'e' || *r == 'E')) ||
                        (*r == 'p' || *r == 'P')) {
                        p = r;
                        is_float = true;
                    } else
                        break;  /* Terminate the token */
                } else
                    break;
            }
            p--;        /* Point to first character beyond number */

            if (p == line+1 && *line == '$') {
                type = TOKEN_HERE;
            } else {
                if (has_e && !is_hex) {
                    /* 1e13 is floating-point, but 1e13h is not */
                    is_float = true;
                }

                type = is_float ? TOKEN_FLOAT : TOKEN_NUM;
            }
        } else if (nasm_isspace(*p)) {
            firstchar = ' ';    /* Always a single space */
            type = TOKEN_WHITESPACE;
            p = nasm_skip_spaces(p);
            /*
             * Whitespace just before end-of-line is discarded by
             * pretending it's a comment; whitespace just before a
             * comment gets lumped into the comment.
             */
            if (!*p || *p == ';')
                type = TOKEN_EOS;
        } else if (*p == ';') {
            type = TOKEN_EOS;
        } else {
            /*
             * Anything else is an operator of some kind. We check
             * for all the double-character operators (>>, <<, //,
             * %%, <=, >=, ==, !=, <>, &&, ||, ^^) and the triple-
	     * character operators (<<<, >>>, <=>) but anything
             * else is a single-character operator.
             */
            type = (unsigned char)*p;
	    switch (*p++) {
	    case '>':
		if (*p == '>') {
		    p++;
                    type = TOKEN_SHR;
		    if (*p == '>') {
                        type = TOKEN_SAR;
			p++;
                    }
		} else if (*p == '=') {
                    type = TOKEN_GE;
                    p++;
                }
		break;

	    case '<':
		if (*p == '<') {
		    p++;
                    type = TOKEN_SHL;
		    if (*p == '<')
			p++;
		} else if (*p == '=') {
		    p++;
                    type = TOKEN_LE;
		    if (*p == '>') {
			p++;
                        type = TOKEN_LEG;
                    }
		} else if (*p == '>') {
		    p++;
                    type = TOKEN_NE;
		}
		break;

	    case '!':
		if (*p == '=') {
		    p++;
                    type = TOKEN_NE;
                }
		break;

	    case '/':
                if (*p == '/') {
                    p++;
                    type = TOKEN_SDIV;
                }
                break;
	    case '=':
                if (*p == '=')
                    p++;        /* Still TOKEN_EQ == '=' though */
                break;
	    case '&':
                if (*p == '&') {
                    p++;
                    type = TOKEN_DBL_AND;
                }
                break;

	    case '|':
                if (*p == '|') {
                    p++;
                    type = TOKEN_DBL_OR;
                }
                break;

	    case '^':
                if (*p == '^') {
                    p++;
                    type = TOKEN_DBL_XOR;
                }
		break;

	    default:
		break;
	    }
        }

        if (type == TOKEN_EOS)
            break;              /* done with the string... */

        if (!ep)
            ep = p;
        toklen = ep - line;

        if (toklen) {
            *tail = t = new_Token(NULL, type, line, toklen);
            *tok_text_buf(t) = firstchar; /* E.g. %{foo} -> {foo -> %foo */
            tail = &t->next;
        }

        line = p;
    }
    return list;
}

/*
 * Tokens are allocated in blocks to improve speed. Set the blocksize
 * to 0 to use regular nasm_malloc(); this is useful for debugging.
 *
 * alloc_Token() returns a zero-initialized token structure.
 */
#define TOKEN_BLOCKSIZE 4096

#if TOKEN_BLOCKSIZE

static Token *freeTokens  = NULL;
static Token *tokenblocks = NULL;

static Token *alloc_Token(void)
{
    Token *t = freeTokens;

    if (unlikely(!t)) {
        Token *block;
        size_t i;

        nasm_newn(block, TOKEN_BLOCKSIZE);

        /*
         * The first entry in each array are a linked list of
         * block allocations and is not used for data.
         */
        block[0].next = tokenblocks;
	block[0].type = TOKEN_BLOCK;
        tokenblocks = block;

        /*
         * Add the rest to the free list
         */
        for (i = 2; i < TOKEN_BLOCKSIZE - 1; i++)
            block[i].next = &block[i+1];

        freeTokens = &block[2];

        /*
         * Return the topmost usable token
         */
        return &block[1];
    }

    freeTokens = t->next;
    t->next = NULL;
    return t;
}

static Token *delete_Token(Token *t)
{
    Token *next;

    nasm_assert(t && t->type != TOKEN_FREE);

    next = t->next;
    nasm_zero(*t);
    t->type = TOKEN_FREE;
    t->next = freeTokens;
    freeTokens = t;

    return next;
}

static void delete_Blocks(void)
{
    Token *block, *blocktmp;

    list_for_each_safe(block, blocktmp, tokenblocks)
        nasm_free(block);

    freeTokens = tokenblocks = NULL;
}

#else

static inline Token *alloc_Token(void)
{
    Token *t;
    nasm_new(t);
    return t;
}

static Token *delete_Token(Token *t)
{
    Token *next = t->next;
    nasm_free(t);
    return next;
}

static inline void delete_Blocks(void)
{
    /* Nothing to do */
}

#endif

/*
 *  this function creates a new Token and passes a pointer to it
 *  back to the caller.  It sets the type, text, and next pointer elements.
 */
static Token *new_Token(Token * next, enum token_type type,
                        const char *text, size_t txtlen)
{
    Token *t = alloc_Token();
    char *textp;

    t->next = next;
    t->type = type;
    if (type == TOKEN_WHITESPACE) {
        t->len = 1;
        t->text.a[0] = ' ';
    } else {
        if (text && text[0] && !txtlen)
            txtlen = tok_strlen(text);

        t->len = tok_check_len(txtlen);

        if (text) {
            textp = (txtlen > INLINE_TEXT)
                ? (t->text.p.ptr = nasm_malloc(txtlen+1)) : t->text.a;
            memcpy(textp, text, txtlen);
            textp[txtlen] = '\0';   /* In case we needed malloc() */
        } else {
            /*
             * Allocate a buffer but do not fill it. The caller
             * can fill in text, but must not change the length.
             * The filled in text must be exactly txtlen once
             * the buffer is filled and before the token is added
             * to any line lists.
             */
            if (txtlen > INLINE_TEXT)
                t->text.p.ptr = nasm_zalloc(txtlen+1);
        }
    }
    return t;
}

/*
 * Same as new_Token(), but text belongs to the new token and is
 * either taken over or freed.  This function MUST be called
 * with valid txt and txtlen, unlike new_Token().
 */
static Token *new_Token_free(Token * next, enum token_type type,
                             char *text, size_t txtlen)
{
    Token *t = alloc_Token();

    t->next = next;
    t->type = type;
    t->len = tok_check_len(txtlen);

    if (txtlen <= INLINE_TEXT) {
        memcpy(t->text.a, text, txtlen);
        nasm_free(text);
    } else {
        t->text.p.ptr = text;
    }

    return t;
}

static Token *dup_Token(Token *next, const Token *src)
{
    Token *t = alloc_Token();

    memcpy(t, src, sizeof *src);
    t->next = next;

    if (t->len > INLINE_TEXT) {
        t->text.p.ptr = nasm_malloc(t->len + 1);
        memcpy(t->text.p.ptr, src->text.p.ptr, t->len+1);
    }

    return t;
}

static Token *new_White(Token *next)
{
    Token *t = alloc_Token();

    t->next = next;
    t->type = TOKEN_WHITESPACE;
    t->len  = 1;
    t->text.a[0] = ' ';

    return t;
}

/*
 * This *transfers* the content from one token to another, leaving the
 * next pointer of the latter intact. Unlike dup_Token(), the old
 * token is destroyed, except for its next pointer, and the text
 * pointer allocation, if any, is simply transferred.
 */
static Token *steal_Token(Token *dst, Token *src)
{
    /* Overwrite everything except the next pointers */
    memcpy((char *)dst + sizeof(Token *), (char *)src + sizeof(Token *),
	   sizeof(Token) - sizeof(Token *));

    /* Clear the donor token */
    memset((char *)src + sizeof(Token *), 0, sizeof(Token) - sizeof(Token *));

    return dst;
}

/*
 * Convert a line of tokens back into text. This modifies the list
 * by expanding environment variables.
 *
 * If expand_locals is not zero, identifiers of the form "%$*xxx"
 * are also transformed into ..@ctxnum.xxx
 */
static char *detoken(Token * tlist, bool expand_locals)
{
    Token *t;
    char *line, *p;
    int len = 0;

    list_for_each(t, tlist) {
	switch (t->type) {
	case TOKEN_ENVIRON:
	{
	    const char *v = pp_getenv(t, true);
	    set_text(t, v, tok_strlen(v));
	    t->type = TOKEN_NAKED_STR;
	    break;
        }

	case TOKEN_LOCAL_MACRO:
        case TOKEN_LOCAL_SYMBOL:
	    if (expand_locals) {
		const char *q;
		char *p;
		Context *ctx = get_ctx(tok_text(t), &q);
		if (ctx) {
		    p = nasm_asprintf("..@%"PRIu64".%s", ctx->number, q);
		    set_text_free(t, p, nasm_last_string_len());
		    t->type = TOKEN_ID;
		}
	    }
	    break;

        case TOKEN_INDIRECT:
            /*
             * This won't happen in when emitting to the assembler,
             * but can happen when emitting output for some of the
             * list options. The token string doesn't actually include
             * the brackets in this case.
             */
            len += 3;           /* %[] */
            break;

	default:
	    break;		/* No modifications */
        }

        if (debug_level(2)) {
            unsigned int t_len  = t->len;
            unsigned int s_len = tok_strlen(tok_text(t));
            if (t_len != s_len) {
                nasm_panic("assertion failed: token \"%s\" type %u len %u has t->len %u\n",
                           tok_text(t), t->type, s_len, t_len);
                t->len = s_len;
            }
        }

	len += t->len;
    }

    p = line = nasm_malloc(len + 1);

    list_for_each(t, tlist) {
        switch (t->type) {
        case TOKEN_INDIRECT:
            *p++ = '%';
            *p++ = '[';
            p = mempcpy(p, tok_text(t), t->len);
            *p++ = ']';
            break;

        default:
            p = mempcpy(p, tok_text(t), t->len);
        }
    }
    *p = '\0';

    return line;
}

/*
 * A scanner, suitable for use by the expression evaluator, which
 * operates on a line of Tokens. Expects a pointer to a pointer to
 * the first token in the line to be passed in as its private_data
 * field.
 *
 * FIX: This really needs to be unified with stdscan.
 */
struct ppscan {
    Token *tptr;
    int ntokens;
};

static int ppscan(void *private_data, struct tokenval *tokval)
{
    struct ppscan *pps = private_data;
    Token *tline;
    const char *txt;

    do {
	if (pps->ntokens && (tline = pps->tptr)) {
	    pps->ntokens--;
	    pps->tptr = tline->next;
	} else {
	    pps->tptr = NULL;
	    pps->ntokens = 0;
	    return tokval->t_type = TOKEN_EOS;
	}
    } while (tline->type == TOKEN_WHITESPACE);

    txt = tok_text(tline);
    tokval->t_charptr = (char *)txt; /* Fix this */

    switch (tline->type) {
    default:
        return tokval->t_type = tline->type;

    case TOKEN_ID:
        /* This could be an assembler keyword */
	return nasm_token_hash(txt, tokval);

    case TOKEN_NUM:
    {
        bool rn_error;
        tokval->t_integer = readnum(txt, &rn_error);
        if (rn_error)
            return tokval->t_type = TOKEN_ERRNUM;
        else
            return tokval->t_type = TOKEN_NUM;
    }

    case TOKEN_STR:
	tokval->t_charptr = (char *)unquote_token(tline);
        /* fall through */
    case TOKEN_INTERNAL_STR:
    case TOKEN_NAKED_STR:
        tokval->t_inttwo = tline->len;
	return tokval->t_type = TOKEN_STR;
    }
}

/*
 * 1. An expression (true if nonzero 0)
 * 2. The keywords true, on, yes for true
 * 3. The keywords false, off, no for false
 * 4. An empty line, for true
 *
 * On error, return defval (usually the previous value)
 */
static bool pp_get_boolean_option(Token *tline, bool defval)
{
    static const char * const noyes[] = {
        "no", "yes",
        "false", "true",
        "off", "on"
    };
    struct ppscan pps;
    struct tokenval tokval;
    expr *evalresult;

    tline = skip_white(tline);
    if (!tline)
        return true;

    if (tline->type == TOKEN_ID) {
        size_t i;
	const char *txt = tok_text(tline);

        for (i = 0; i < ARRAY_SIZE(noyes); i++)
            if (!nasm_stricmp(txt, noyes[i]))
                return i & 1;
    }

    pps.tptr = NULL;
    pps.tptr = tline;
    pps.ntokens = -1;
    tokval.t_type = TOKEN_INVALID;
    evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);

    if (!evalresult)
        return true;

    if (tokval.t_type) {
        /*!
         *!pp-trailing [on] trailing garbage ignored
         *!  warns that the preprocessor encountered additional text
         *!  where no such text was expected. This can
         *!  sometimes be the result of an incorrectly written expression,
         *!  or arguments that are inadvertently separated.
         */
        nasm_warn(WARN_PP_TRAILING,
                  "trailing garbage after expression ignored");
    }
    if (!is_really_simple(evalresult)) {
        nasm_nonfatal("boolean flag expression must be a constant");
        return defval;
    }

    return reloc_value(evalresult) != 0;
}

/*
 * Compare a string to the name of an existing macro; this is a
 * simple wrapper which calls either strcmp or nasm_stricmp
 * depending on the value of the `casesense' parameter.
 */
static int mstrcmp(const char *p, const char *q, bool casesense)
{
    return casesense ? strcmp(p, q) : nasm_stricmp(p, q);
}

/*
 * Compare a string to the name of an existing macro; this is a
 * simple wrapper which calls either strcmp or nasm_stricmp
 * depending on the value of the `casesense' parameter.
 */
static int mmemcmp(const char *p, const char *q, size_t l, bool casesense)
{
    return casesense ? memcmp(p, q, l) : nasm_memicmp(p, q, l);
}

/*
 * Return the Context structure associated with a %$ token. Return
 * NULL, having _already_ reported an error condition, if the
 * context stack isn't deep enough for the supplied number of $
 * signs.
 *
 * If "namep" is non-NULL, set it to the pointer to the macro name
 * tail, i.e. the part beyond %$...
 */
static Context *get_ctx(const char *name, const char **namep)
{
    Context *ctx;
    int i;

    if (namep)
        *namep = name;

    if (!name || name[0] != '%' || name[1] != '$')
        return NULL;

    if (!cstk) {
        nasm_nonfatal("`%s': context stack is empty", name);
        return NULL;
    }

    name += 2;
    ctx = cstk;
    i = 0;
    while (ctx && *name == '$') {
        name++;
        i++;
        ctx = ctx->next;
    }
    if (!ctx) {
        nasm_nonfatal("`%s': context stack is only"
                      " %d level%s deep", name, i, (i == 1 ? "" : "s"));
        return NULL;
    }

    if (namep)
        *namep = name;

    return ctx;
}

/*
 * Open an include file. This routine must always return a valid
 * file pointer if it returns - it's responsible for throwing an
 * ERR_FATAL and bombing out completely if not. It should also try
 * the include path one by one until it finds the file or reaches
 * the end of the path.
 *
 * Note: for INC_PROBE the function returns NULL at all times;
 * instead look for a filename in *slpath.
 */
enum incopen_mode {
    INC_NEEDED,                 /* File must exist */
    INC_REQUIRED,               /* File must exist, but only open once/pass */
    INC_OPTIONAL,               /* Missing is OK */
    INC_PROBE                   /* Only an existence probe */
};

/* This is conducts a full pathname search */
static FILE *inc_fopen_search(const char *file, char **slpath,
                              enum incopen_mode omode, enum file_flags fmode)
{
    const struct strlist_entry *ip = strlist_head(ipath_list);
    FILE *fp;
    const char *prefix = "";
    char *sp;
    bool found;

    while (1) {
        sp = nasm_catfile(prefix, file);
        if (omode == INC_PROBE) {
            fp = NULL;
            found = nasm_file_exists(sp);
        } else {
            fp = nasm_open_read(sp, fmode);
            found = (fp != NULL);
        }
        if (found) {
            *slpath = sp;
            return fp;
        }

        nasm_free(sp);

        if (!ip) {
            *slpath = NULL;
            return NULL;
        }

        prefix = ip->str;
        ip = ip->next;
    }
}

/*
 * Open a file, or test for the presence of one (depending on omode),
 * considering the include path.
 */
struct file_hash_entry {
    const char *path;
    struct file_hash_entry *full; /* Hash entry for the full path */
    int64_t include_pass; /* Pass in which last included (for %require) */
};

static FILE *inc_fopen(const char *file,
                       struct strlist *dhead,
                       const char **found_path,
                       enum incopen_mode omode,
                       enum file_flags fmode)
{
    struct file_hash_entry **fhep;
    struct file_hash_entry *fhe = NULL;
    struct hash_insert hi;
    const char *path = NULL;
    FILE *fp = NULL;
    const int64_t pass = pass_count();
    bool skip_open = (omode == INC_PROBE);

    fhep = (struct file_hash_entry **)hash_find(&FileHash, file, &hi);
    if (fhep) {
        fhe = *fhep;
        if (fhe) {
            path = fhe->path;
            skip_open |= (omode == INC_REQUIRED) &&
                (fhe->full->include_pass >= pass);
        }
    } else {
        /* Need to do the actual path search */
        char *pptr;
        fp = inc_fopen_search(file, &pptr, omode, fmode);
        path = pptr;

        /* Positive or negative result */
        if (path) {
            nasm_new(fhe);
            fhe->path = path;
            fhe->full = fhe;    /* It is *possible*... */
        }
        hash_add(&hi, nasm_strdup(file), fhe);

        /*
         * Add a hash entry for the canonical path if there isn't one
         * already. Try to get the unique name from the OS best we can.
         * Note that ->path and ->full->path can be different, and that
         * is okay (we don't want to print out a full canonical path
         * in messages, for example.)
         */
        if (path) {
            char *fullpath = nasm_realpath(path);

            if (!strcmp(file, fullpath)) {
                nasm_free(fullpath);
            } else {
                struct file_hash_entry **fullp, *full;
                fullp = (struct file_hash_entry **)
                    hash_find(&FileHash, fullpath, &hi);

                if (fullp) {
                    full = *fullp;
                    nasm_free(fullpath);
                } else {
                    nasm_new(full);
                    full->path = fullpath;
                    full->full = full;
                    hash_add(&hi, path, full);
                }
                fhe->full = full;
            }
        }

        /*
         * Add file to dependency path.
         */
        strlist_add(dhead, path ? path : file);
    }

    if (path && !fp && omode != INC_PROBE)
        fp = nasm_open_read(path, fmode);

    if (omode < INC_OPTIONAL && !fp) {
        if (!path)
            errno = ENOENT;

        nasm_nonfatal("unable to open include file `%s': %s",
                      file, strerror(errno));
    }

    if (fp)
        fhe->full->include_pass = pass;

    if (found_path)
        *found_path = path;

    return fp;
}

/*
 * Opens an include or input file. Public version, for use by modules
 * that get a file:lineno pair and need to look at the file again
 * (e.g. the CodeView debug backend). Returns NULL on failure.
 */
FILE *pp_input_fopen(const char *filename, enum file_flags mode)
{
    return inc_fopen(filename, NULL, NULL, INC_OPTIONAL, mode);
}

/*
 * Determine if we should warn on defining a single-line macro of
 * name `name', with `nparam' parameters. If nparam is 0 or -1, will
 * return true if _any_ single-line macro of that name is defined.
 * Otherwise, will return true if a single-line macro with either
 * `nparam' or no parameters is defined.
 *
 * If a macro with precisely the right number of parameters is
 * defined, or nparam is -1, the address of the definition structure
 * will be returned in `defn'; otherwise NULL will be returned. If `defn'
 * is NULL, no action will be taken regarding its contents, and no
 * error will occur.
 *
 * Note that this is also called with nparam zero to resolve
 * `ifdef'.
 */
static bool
smacro_defined(Context *ctx, const char *name, int nparam, SMacro **defn,
               bool nocase, bool find_alias)
{
    struct hash_table *smtbl;
    SMacro *m;

    smtbl = ctx ? &ctx->localmac : &smacros;

restart:
    m = (SMacro *) hash_findix(smtbl, name);

    while (m) {
        if (!mstrcmp(m->name, name, m->casesense && nocase) &&
            (nparam <= 0 || m->nparam == 0 ||
             (nparam >= m->nparam_min &&
              (m->varadic || nparam <= m->nparam)))) {
            if (m->alias && !find_alias) {
                if (!ppconf.noaliases) {
                    name = tok_text(m->expansion);
                    goto restart;
                } else {
                    continue;
                }
            }
            if (defn)
                *defn = m;
            return true;
        }
        m = m->next;
    }

    return false;
}

/* param should be a natural number [0; INT_MAX] */
static int read_param_count(const char *str)
{
    int result;
    bool err;

    result = readnum(str, &err);
    if (result < 0 || result > INT_MAX) {
        result = 0;
        nasm_nonfatal("parameter count `%s' is out of bounds [%d; %d]",
                      str, 0, INT_MAX);
    } else if (err)
        nasm_nonfatal("unable to parse parameter count `%s'", str);
    return result;
}

/*
 * Count and mark off the parameters in a multi-line macro call.
 * This is called both from within the multi-line macro expansion
 * code, and also to mark off the default parameters when provided
 * in a %macro definition line.
 *
 * Note that we need space in the params array for parameter 0 being
 * a possible captured label as well as the final NULL.
 *
 * Returns a pointer to the pointer to a terminal comma if present;
 * used to drop an empty terminal argument for legacy reasons.
 */
static Token **count_mmac_params(Token *tline, int *nparamp, Token ***paramsp)
{
    int paramsize;
    int nparam = 0;
    Token *t;
    Token **comma = NULL, **maybe_comma = NULL;
    Token **params;

    paramsize = PARAM_DELTA;
    nasm_newn(params, paramsize);

    t = skip_white(tline);
    if (t) {
        while (true) {
            /* Need two slots for captured label and NULL */
            if (unlikely(nparam+2 >= paramsize)) {
                paramsize += PARAM_DELTA;
                params = nasm_realloc(params, sizeof(*params) * paramsize);
            }
            params[++nparam] = t;
            if (tok_is(t, '{')) {
                int brace = 1;

                comma = NULL;   /* Non-empty parameter */

                while (brace && (t = t->next)) {
                    brace += tok_is(t, '{');
                    brace -= tok_is(t, '}');
                }

                if (t) {
                    /*
                     * Now we've found the closing brace, look further
                     * for the comma.
                     */
                    t = skip_white(t->next);
                    if (tok_isnt(t, ','))
                        nasm_nonfatal("braces do not enclose all of macro parameter");
                } else {
                    nasm_nonfatal("expecting closing brace in macro parameter");
                }
            }

            /* Advance to the next comma */
            maybe_comma = &t->next;
            while (tok_isnt(t, ',')) {
                if (!tok_white(t))
                    comma = NULL; /* Non-empty parameter */
                maybe_comma = &t->next;
                t = t->next;
            }

            if (!t)
                break;              /* End of string, no comma */

            comma = maybe_comma;     /* Point to comma pointer */
            t = skip_white(t->next); /* Eat the comma and whitespace */
        }
    }

    params[nparam+1] = NULL;
    *paramsp = params;
    *nparamp = nparam;

    return comma;
}

/*
 * Determine whether one of the various `if' conditions is true or
 * not.
 *
 * We must free the tline we get passed.
 */
static enum cond_state if_condition(Token * tline, enum preproc_token ct)
{
    bool j;
    Token *t, *tt, *origline;
    struct ppscan pps;
    struct tokenval tokval;
    expr *evalresult;
    enum token_type needtype;
    const char *dname = pp_directives[ct];
    bool casesense = true;
    enum preproc_token cond = PP_COND(ct);

    origline = tline;

    switch (cond) {
    case PP_IFCTX:
        j = false;              /* have we matched yet? */
        while (true) {
            tline = skip_white(tline);
            if (!tline)
                break;
            if (tline->type != TOKEN_ID) {
                nasm_nonfatal("`%s' expects context identifiers",
                              dname);
                goto fail;
            }
            if (cstk && cstk->name && !nasm_stricmp(tok_text(tline), cstk->name))
                j = true;
            tline = tline->next;
        }
        break;

    case PP_IFDEF:
    case PP_IFDEFALIAS:
    {
        bool alias = cond == PP_IFDEFALIAS;
        SMacro *smac;
        Context *ctx;
        const char *mname;

        j = false;              /* have we matched yet? */
        while (tline) {
            tline = skip_white(tline);
            if (!tline || (tline->type != TOKEN_ID &&
			   tline->type != TOKEN_LOCAL_MACRO)) {
                nasm_nonfatal("`%s' expects macro identifiers",
                              dname);
                goto fail;
            }

            mname = tok_text(tline);
            ctx = get_ctx(mname, &mname);
            if (smacro_defined(ctx, mname, -1, &smac, true, alias) && smac
                && smac->alias == alias) {
                j = true;
                break;
            }
            tline = tline->next;
        }
        break;
    }

    case PP_IFDIFI:
        /*
         * %ifdifi doesn't actually exist; it ignores its argument and is
         * always false. This exists solely to stub out the corresponding
         * TASM directive.
         */
        j = false;
        goto fail;

    case PP_IFENV:
        tline = expand_smacro(tline);
        j = false;              /* have we matched yet? */
        while (tline) {
            tline = skip_white(tline);
            if (!tline || (tline->type != TOKEN_ID &&
                           tline->type != TOKEN_STR &&
			   tline->type != TOKEN_INTERNAL_STR &&
                           tline->type != TOKEN_ENVIRON)) {
                nasm_nonfatal("`%s' expects environment variable names",
                              dname);
                goto fail;
            }

	    j |= !!pp_getenv(tline, false);
            tline = tline->next;
	}
	break;

    case PP_IFIDNI:
        casesense = false;
        /* fall through */
    case PP_IFIDN:
        tline = expand_smacro(tline);
        t = tt = tline;
        while (tok_isnt(tt, ','))
            tt = tt->next;
        if (!tt) {
            nasm_nonfatal("`%s' expects two comma-separated arguments",
                          dname);
            goto fail;
        }
        tt = tt->next;
        j = true;               /* assume equality unless proved not */
        while (tok_isnt(t, ',') && tt) {
	    unsigned int l1, l2;
	    const char *t1, *t2;

            if (tok_is(tt, ',')) {
                nasm_nonfatal("`%s': more than one comma on line",
                              dname);
                goto fail;
            }
            if (t->type == TOKEN_WHITESPACE) {
                t = t->next;
                continue;
            }
            if (tt->type == TOKEN_WHITESPACE) {
                tt = tt->next;
                continue;
            }
            if (tt->type != t->type) {
                j = false;      /* found mismatching tokens */
                break;
            }

	    t1 = unquote_token(t);
	    t2 = unquote_token(tt);
	    l1 = t->len;
	    l2 = tt->len;

	    if (l1 != l2 || mmemcmp(t1, t2, l1, casesense)) {
		j = false;
		break;
	    }

            t = t->next;
            tt = tt->next;
        }
        if (!tok_is(t, ',') || tt)
            j = false;          /* trailing gunk on one end or other */
        break;

    case PP_IFMACRO:
    {
        bool found = false;
        MMacro searching, *mmac;

        tline = skip_white(tline);
        tline = expand_id(tline);
        if (!tok_is(tline, TOKEN_ID)) {
            nasm_nonfatal("`%s' expects a macro name", dname);
            goto fail;
        }
        nasm_zero(searching);
        searching.name = dup_text(tline);
        searching.casesense = true;
        searching.nparam_min = 0;
        searching.nparam_max = INT_MAX;
        tline = expand_smacro(tline->next);
        tline = skip_white(tline);
        if (!tline) {
        } else if (!tok_is(tline, TOKEN_NUM)) {
            nasm_nonfatal("`%s' expects a parameter count or nothing",
                          dname);
        } else {
            searching.nparam_min = searching.nparam_max =
                read_param_count(tok_text(tline));
        }
        if (tline && tok_is(tline->next, '-')) {
            tline = tline->next->next;
            if (tok_is(tline, '*'))
                searching.nparam_max = INT_MAX;
            else if (!tok_is(tline, TOKEN_NUM))
                nasm_nonfatal("`%s' expects a parameter count after `-'",
                              dname);
            else {
                searching.nparam_max = read_param_count(tok_text(tline));
                if (searching.nparam_min > searching.nparam_max) {
                    nasm_nonfatal("minimum parameter count exceeds maximum");
                    searching.nparam_max = searching.nparam_min;
                }
            }
        }
        if (tline && tok_is(tline->next, '+')) {
            tline = tline->next;
            searching.plus = true;
        }
        mmac = (MMacro *) hash_findix(&mmacros, searching.name);
        while (mmac) {
            if (!strcmp(mmac->name, searching.name) &&
                (mmac->nparam_min <= searching.nparam_max
                 || searching.plus)
                && (searching.nparam_min <= mmac->nparam_max
                    || mmac->plus)) {
                found = true;
                break;
            }
            mmac = mmac->next;
        }
        if (tline && tline->next) {
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after `%s' ignored", dname);
        }
        nasm_free(searching.name);
        j = found;
        break;
    }

    case PP_IFID:
        needtype = TOKEN_ID;
        goto iftype;
    case PP_IFNUM:
        needtype = TOKEN_NUM;
        goto iftype;
    case PP_IFSTR:
        needtype = TOKEN_STR;
        goto iftype;

iftype:
        t = tline = expand_smacro(tline);

        while (tok_white(t) ||
               (needtype == TOKEN_NUM && (tok_is(t, '-') || tok_is(t, '+'))))
            t = t->next;

        j = tok_is(t, needtype);
        break;

    case PP_IFTOKEN:
        tline = expand_smacro(tline);
        t = skip_white(tline);

        j = false;
        if (t) {
            t = skip_white(t->next); /* Skip the actual token + whitespace */
            j = !t;
        }
        break;

    case PP_IFEMPTY:
        tline = expand_smacro(tline);
        t = skip_white(tline);
        j = !t;                 /* Should be empty */
        break;

    case PP_IF:
        pps.tptr = tline = expand_smacro(tline);
	pps.ntokens = -1;
        tokval.t_type = TOKEN_INVALID;
        evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
        if (!evalresult)
            return -1;
        if (tokval.t_type) {
            nasm_warn(WARN_PP_TRAILING, "trailing garbage after expression ignored");
        }
        if (!is_simple(evalresult)) {
            nasm_nonfatal("non-constant value given to `%s'",
                          dname);
            goto fail;
        }
        j = reloc_value(evalresult) != 0;
        break;

    case PP_IFUSING:
    case PP_IFUSABLE:
    {
        const struct use_package *pkg;
        const char *name;

        pkg = get_use_pkg(tline, dname, &name);
        if (!name)
            goto fail;

        j = pkg && ((cond == PP_IFUSABLE) | use_loaded[pkg->index]);
        break;
    }

    default:
        nasm_nonfatal("unknown preprocessor directive `%s'", dname);
        goto fail;
    }

    free_tlist(origline);
    return (j ^ PP_COND_NEGATIVE(ct)) ? COND_IF_TRUE : COND_IF_FALSE;

fail:
    free_tlist(origline);
    return COND_NEVER;
}

/*
 * Default smacro expansion routine: just returns a copy of the
 * expansion list.
 */
static Token *
smacro_expand_default(const SMacro *s, Token **params, int nparams)
{
    (void)params;
    (void)nparams;

    return dup_tlist(s->expansion, NULL);
}

/*
 * Emit a macro definition or undef to the listing file or debug format
 * if desired. This is similar to detoken(), but it handles the
 * reverse expansion list, does not expand %! or local variable
 * tokens, and does some special handling for macro parameters.
 */
static void
list_smacro_def(enum preproc_token op, const Context *ctx, const SMacro *m)
{
    Token *t;
    size_t namelen, size;
    char *def, *p, *end_spec;
    char *context_prefix = NULL;
    size_t context_len;

    namelen = strlen(m->name);
    size = namelen + 2;  /* Include room for space after name + NUL */

    if (ctx) {
        int context_depth = cstk->depth - ctx->depth + 1;
        context_prefix =
            nasm_asprintf("[%s::%"PRIu64"] %%%-*s",
                          ctx->name ? ctx->name : "",
                          ctx->number, context_depth, "");

        context_len = nasm_last_string_len();
        memset(context_prefix + context_len - context_depth,
               '$', context_depth);
        size += context_len;
    }

    list_for_each(t, m->expansion)
        size += t->len;

    if (m->nparam) {
        /*
         * Space for ( and either , or ) around each
         * parameter, plus up to 5 flags + /ux
         */
        int i;

        size += 1 + 8 * m->nparam;
        for (i = 0; i < m->nparam; i++)
            size += m->params[i].name.len;
    }

    def = nasm_malloc(size);
    p = def+size;
    *--p = '\0';

    list_for_each(t, m->expansion) {
	p -= t->len;
	memcpy(p, tok_text(t), t->len);
    }

    *--p = ' ';
    end_spec = p;               /* Truncate here for macro def only */

    if (m->nparam) {
        int i;

        *--p = ')';

        for (i = m->nparam-1; i >= 0; i--) {
            enum sparmflags flags = m->params[i].flags;
            bool slash = false;

            if (m->params[i].radix) {
                *--p = m->params[i].radix;
                slash = true;
            }
            if (flags & SPARM_UNSIGNED) {
                *--p = 'u';
                slash = true;
            }
            if (slash)
                *--p = '/';

            if (flags & (SPARM_GREEDY|SPARM_VARADIC))
                *--p = '+';
	    p -= m->params[i].name.len;
	    memcpy(p, tok_text(&m->params[i].name), m->params[i].name.len);

            if (flags & SPARM_NOSTRIP)
                *--p = '!';
            if (flags & SPARM_STR) {
                *--p = '&';
                if (flags & SPARM_CONDQUOTE)
                    *--p = '&';
            }
            if (flags & SPARM_EVAL)
                *--p = '=';
            *--p = ',';
        }
        *p = '(';               /* First parameter starts with ( not , */
    }

    p -= namelen;
    memcpy(p, m->name, namelen);

    if (context_prefix) {
        p -= context_len;
        memcpy(p, context_prefix, context_len);
        nasm_free(context_prefix);
    }

    if (ppdbg & PDBG_LIST_SMACROS)
        nasm_listmsg("%s %s", pp_directives[op], p);
    if (ppdbg & PDBG_SMACROS) {
        bool define = !(op == PP_UNDEF || op == PP_UNDEFALIAS);
        if (!define)
            *end_spec = '\0';   /* Remove the expansion (for list file only) */
        dfmt->debug_smacros(define, p);
    }
    nasm_free(def);
}

/*
 * Parse smacro arguments, return argument count. If the tmpl argument
 * is set, set the nparam, varadic and params field in the template.
 * The varadic field is not used by define_smacro(), but is provided
 * in case the caller wants it for other purposes.
 *
 * *tpp is updated to point to the pointer to the first token after the
 * prototype.
 *
 * The text values from any argument tokens are "stolen" and the
 * corresponding text fields set to NULL.
 *
 * Note that the user can't define a true varadic macro; doing so
 * would be meaningless. The true varadic macros are only used for
 * internal "magic macro" functions.
 */
static int parse_smacro_template(Token ***tpp, SMacro *tmpl)
{
    int nparam = 0;
    enum sparmflags flags;
    struct smac_param *params = NULL;
    bool err, done;
    bool greedy = false;
    bool parsing_radix;
    char radix;
    Token **tn = *tpp;
    Token *t = *tn;
    Token *name;

    /*
     * DO NOT skip whitespace here, or we won't be able to distinguish:
     *
     * %define foo (a,b)		; no arguments, (a,b) is the expansion
     * %define bar(a,b)			; two arguments, empty expansion
     *
     * This ambiguity was inherited from C.
     */

    if (!tok_is(t, '('))
        goto finish;

    if (tmpl) {
        Token *tx = t;
        Token **txpp = &tx;
        int sparam;

        /* Count parameters first */
        sparam = parse_smacro_template(&txpp, NULL);
        if (!sparam)
            goto finish;        /* No parameters, we're done */
        nasm_newn(params, sparam);
    }

    /* Skip leading paren */
    tn = &t->next;
    t = *tn;

    name = NULL;
    flags = 0;
    radix = 0;
    parsing_radix = false;
    err = done = false;

    while (!done) {
        if (!t) {
            if (name || flags)
                nasm_nonfatal("`)' expected to terminate macro template");
            else
                nasm_nonfatal("parameter identifier expected");
            break;
        }

        switch (t->type) {
        case TOKEN_ID:
            if (parsing_radix) {
                const char *cp;
                for (cp = tok_text(t); cp && *cp; cp++) {
                    switch (*cp | 0x20) {
                    case 'b': case 'y':
                    case 'd': case 't':
                    case 'o': case 'q':
                    case 'h': case 'x':
                        radix = *cp;
                        break;
                    case 's':
                        flags &= ~SPARM_UNSIGNED;
                        break;
                    case 'u':
                        flags |= SPARM_UNSIGNED;
                        break;
                    default:
                        nasm_nonfatal("invalid radix specifier `/%s'",
                                      tok_text(t));
                        cp = NULL;
                        break;
                    }
                }
            } else {
                if (name)
                    goto bad;
                name = t;
            }
            break;
        case '=':
            flags |= SPARM_EVAL;
            break;
        case '&':
            flags |= SPARM_STR;
            break;
        case TOKEN_DBL_AND:
            flags |= SPARM_STR|SPARM_CONDQUOTE;
            break;
        case '!':
            flags |= SPARM_NOSTRIP;
            break;
        case '+':
            flags |= SPARM_GREEDY|SPARM_OPTIONAL;
            greedy = true;
            break;
        case '/':
            if (!(flags & SPARM_EVAL))
                nasm_nonfatal("radix specifier for parameter without `='");
            parsing_radix = true;
            break;
        case ',':
            if (greedy)
                nasm_nonfatal("greedy parameter must be last");
            goto end_param;
        case ')':
            done = true;
            goto end_param;
        end_param:
            if (params) {
                if (name)
                    steal_Token(&params[nparam].name, name);
                params[nparam].flags = flags;
                params[nparam].radix = radix;
            }
            nparam++;
            name = NULL;
            flags = 0;
            parsing_radix = false;
            radix = 0;
            break;
        case TOKEN_WHITESPACE:
            break;
        default:
        bad:
            if (!err) {
                nasm_nonfatal("garbage `%s' in macro parameter list",
                              tok_text(t));
                err = true;
            }
            break;
        }

        tn = &t->next;
        t = *tn;
    }

finish:
    while (t && t->type == TOKEN_WHITESPACE) {
        tn = &t->next;
        t = t->next;
    }
    *tpp = tn;
    if (tmpl) {
        tmpl->nparam     = nparam;
        tmpl->varadic    = greedy;
        tmpl->params     = params;
    }
    return nparam;
}

/*
 * Common code for defining an smacro. The tmpl argument, if not NULL,
 * contains any macro parameters that aren't explicit arguments;
 * those are the more uncommon macro variants.
 */
static SMacro *define_smacro(const char *mname, bool casesense,
                             Token *expansion, SMacro *tmpl)
{
    SMacro *smac, **smhead;
    struct hash_table *smtbl;
    Context *ctx;
    bool defining_alias = false;
    int nparam = 0;
    bool defined;

    if (tmpl) {
        defining_alias = tmpl->alias;
        nparam = tmpl->nparam;
        if (nparam && !defining_alias)
            mark_smac_params(expansion, tmpl, 0);
    }

    ctx = get_ctx(mname, &mname);

    defined = smacro_defined(ctx, mname, nparam, &smac, casesense, true);

    if (defined) {
        if (smac->alias) {
            if (smac->in_progress) {
                nasm_nonfatal("macro alias loop");
                goto fail;
            }

            if (!defining_alias && !ppconf.noaliases) {
                /* It is an alias macro; follow the alias link */
                SMacro *s;

                smac->in_progress++;
                s = define_smacro(tok_text(smac->expansion), casesense,
                                  expansion, tmpl);
                smac->in_progress--;
                return s;
            }
        }

        if (casesense ^ smac->casesense) {
            /*
             *!pp-macro-def-case-single [on] single-line macro defined both case sensitive and insensitive
             *!=macro-def-case-single
             *!  warns when a single-line macro is defined both case
             *!  sensitive and case insensitive.
             *!  The new macro
             *!  definition will override (shadow) the original one,
             *!  although the original macro is not deleted, and will
             *!  be re-exposed if the new macro is deleted with
             *!  \c{%undef}, or, if the original macro is the case
             *!  insensitive one, the macro call is done with a
             *!  different case.
             */
            nasm_warn(WARN_PP_MACRO_DEF_CASE_SINGLE, "case %ssensitive definition of macro `%s' will shadow %ssensitive macro `%s'",
                      casesense ? "" : "in",
                      mname,
                      smac->casesense ? "" : "in",
                      smac->name);
            defined = false;
        } else if ((!!nparam) ^ (!!smac->nparam)) {
            /*
             * Most recent versions of NASM considered this an error,
             * so promote this warning to error by default.
             *
             *!pp-macro-def-param-single [err] single-line macro defined with and without parameters
             *!=macro-def-param-single
             *!  warns if the same single-line macro is defined with and
             *!  without parameters.
             *!  The new macro
             *!  definition will override (shadow) the original one,
             *!  although the original macro is not deleted, and will
             *!  be re-exposed if the new macro is deleted with
             *!  \c{%undef}.
             */
            nasm_warn(WARN_PP_MACRO_DEF_PARAM_SINGLE,
                      "macro `%s' defined both with and without parameters",
                      mname);
            defined = false;
        } else if (smac->nparam < nparam) {
            /*
             *!pp-macro-def-greedy-single [on] single-line macro
             *!=macro-def-greedy-single
             *!  definition shadows greedy macro warns when a
             *!  single-line macro is defined which would match a
             *!  previously existing greedy definition.  The new macro
             *!  definition will override (shadow) the original one,
             *!  although the original macro is not deleted, and will
             *!  be re-exposed if the new macro is deleted with
             *!  \c{%undef}, and will be invoked if called with a
             *!  parameter count that does not match the new definition.
             */
            nasm_warn(WARN_PP_MACRO_DEF_GREEDY_SINGLE,
                      "defining macro `%s' shadows previous greedy definition",
                      mname);
            defined = false;
        }
    }

    if (defined) {
        /*
         * We're redefinining, so we have to take over an
         * existing SMacro structure. This means freeing
         * what was already in it, but not the structure itself.
         */
        clear_smacro(smac);
    } else {
        /* Create a new macro */
        smtbl  = ctx ? &ctx->localmac : &smacros;
        smhead = (SMacro **) hash_findi_add(smtbl, mname);
        nasm_new(smac);
        smac->next = *smhead;
        *smhead = smac;
    }

    smac->name       = nasm_strdup(mname);
    smac->casesense  = casesense;
    smac->expansion  = reverse_tokens(expansion);
    smac->expand     = smacro_expand_default;
    smac->nparam     = nparam;
    smac->nparam_min = nparam;
    if (tmpl) {
        smac->params     = tmpl->params;
        smac->alias      = tmpl->alias;
        smac->recursive  = tmpl->recursive;
        if (tmpl->expand) {
            smac->expand    = tmpl->expand;
            smac->expandpvt = tmpl->expandpvt;
        }
        if (nparam) {
            int nparam_min = nparam;

            smac->varadic =
                !!(tmpl->params[nparam-1].flags &
                   (SPARM_GREEDY|SPARM_VARADIC));

            while (nparam_min > 1) {
                if (!(tmpl->params[nparam_min-1].flags & SPARM_OPTIONAL))
                    break;
                nparam_min--;
            }

            smac->nparam_min = nparam_min;
        }
    }
    if (ppdbg & (PDBG_SMACROS|PDBG_LIST_SMACROS)) {
        list_smacro_def((smac->alias ? PP_DEFALIAS : PP_DEFINE)
                        + !casesense, ctx, smac);
    }
    return smac;

fail:
    free_tlist(expansion);
    if (tmpl)
        free_smacro_members(tmpl);
    return NULL;
}

/*
 * Undefine an smacro
 */
static void undef_smacro(const char *mname, bool undefalias)
{
    SMacro **smhead, *s, **sp;
    struct hash_table *smtbl;
    Context *ctx;

    ctx = get_ctx(mname, &mname);
    smtbl = ctx ? &ctx->localmac : &smacros;
    smhead = (SMacro **)hash_findi(smtbl, mname, NULL);

    if (smhead) {
        /*
         * We now have a macro name... go hunt for it.
         */
        sp = smhead;
        while ((s = *sp) != NULL) {
            if (!mstrcmp(s->name, mname, s->casesense)) {
                if (s->alias && !undefalias) {
                    if (!ppconf.noaliases) {
                        if (s->in_progress) {
                            nasm_nonfatal("macro alias loop");
                        } else {
                            s->in_progress = true;
                            undef_smacro(tok_text(s->expansion), false);
                            s->in_progress = false;
                        }
                    }
                } else {
                    if (list_option('d'))
                        list_smacro_def(s->alias ? PP_UNDEFALIAS : PP_UNDEF,
                                        ctx, s);
                    *sp = s->next;
                    free_smacro(s);
                    continue;
                }
            }
            sp = &s->next;
        }
    }
}

/*
 * Parse a mmacro specification.
 */
static bool parse_mmacro_spec(Token *tline, MMacro *def, const char *directive)
{
    tline = tline->next;
    tline = skip_white(tline);
    tline = expand_id(tline);
    if (!tok_is(tline, TOKEN_ID)) {
        nasm_nonfatal("`%s' expects a macro name", directive);
        return false;
    }

#if 0
    def->prev = NULL;
#endif
    def->name = dup_text(tline);
    def->plus = false;
    def->nolist = 0;
    def->nparam_min = 0;
    def->nparam_max = 0;

    tline = expand_smacro(tline->next);
    tline = skip_white(tline);
    if (!tok_is(tline, TOKEN_NUM))
        nasm_nonfatal("`%s' expects a parameter count", directive);
    else
        def->nparam_min = def->nparam_max = read_param_count(tok_text(tline));
    if (tline && tok_is(tline->next, '-')) {
        tline = tline->next->next;
        if (tok_is(tline, '*')) {
            def->nparam_max = INT_MAX;
        } else if (!tok_is(tline, TOKEN_NUM)) {
            nasm_nonfatal("`%s' expects a parameter count after `-'", directive);
        } else {
            def->nparam_max = read_param_count(tok_text(tline));
            if (def->nparam_min > def->nparam_max) {
                nasm_nonfatal("minimum parameter count exceeds maximum");
                def->nparam_max = def->nparam_min;
            }
        }
    }
    if (tline && tok_is(tline->next, '+')) {
        tline = tline->next;
        def->plus = true;
    }
    if (tline && tok_is(tline->next, TOKEN_ID) &&
	tline->next->len == 7 &&
        !nasm_stricmp(tline->next->text.a, ".nolist")) {
        tline = tline->next;
        if (!list_option('f'))
            def->nolist |= NL_LIST|NL_LINE;
    }

    /*
     * Handle default parameters.
     */
    def->ndefs = 0;
    if (tline && tline->next) {
        Token **comma;
        def->dlist = tline->next;
        tline->next = NULL;
        comma = count_mmac_params(def->dlist, &def->ndefs, &def->defaults);
        if (!ppconf.sane_empty_expansion && comma) {
            *comma = NULL;
            def->ndefs--;
            nasm_warn(WARN_PP_MACRO_PARAMS_LEGACY,
                      "dropping trailing empty default parameter in definition of multi-line macro `%s'",
                      def->name);
        }
    } else {
        def->dlist = NULL;
        def->defaults = NULL;
    }
    def->expansion = NULL;

    if (def->defaults && def->ndefs > def->nparam_max - def->nparam_min &&
        !def->plus) {
        /*
         *!pp-macro-defaults [on] macros with more default than optional parameters
         *!=macro-defaults
         *!  warns when a macro has more default parameters than optional parameters.
         *!  See \k{mlmacdef} for why might want to disable this warning.
         */
        nasm_warn(WARN_PP_MACRO_DEFAULTS,
                   "too many default macro parameters in macro `%s'", def->name);
    }

    return true;
}


/*
 * Decode a size directive
 */
static int parse_size(const char *str) {
    static const char *size_names[] =
        { "byte", "dword", "oword", "qword", "tword", "word", "yword" };
    static const int sizes[] =
        { 0, 1, 4, 16, 8, 10, 2, 32 };
    return str ? sizes[bsii(str, size_names, ARRAY_SIZE(size_names))+1] : 0;
}

/*
 * Process a preprocessor %pragma directive.  Currently there are none.
 * Gets passed the token list starting with the "preproc" token from
 * "%pragma preproc".
 */
static void do_pragma_preproc(Token *tline)
{
    const char *txt;

    /* Skip to the real stuff */
    tline = tline->next;
    tline = skip_white(tline);

    if (!tok_is(tline, TOKEN_ID))
        return;

    txt = tok_text(tline);
    if (!nasm_stricmp(txt, "sane_empty_expansion")) {
        tline = skip_white(tline->next);
        ppconf.sane_empty_expansion =
            pp_get_boolean_option(tline, ppconf.sane_empty_expansion);
    } else {
        /* Unknown pragma, ignore for now */
    }
}

static bool is_macro_id(const Token *t)
{
    return tok_is(t, TOKEN_ID) || tok_is(t, TOKEN_LOCAL_MACRO);
}

static const char *get_id_noskip(Token **tp, const char *dname);

static const char *get_id(Token **tp, const char *dname)
{
    *tp = (*tp)->next;          /* Skip directive */
    return get_id_noskip(tp, dname);
}

static const char *get_id_noskip(Token **tp, const char *dname)
{
    const char *id;
    Token *t = *tp;

    t = skip_white(t);
    t = expand_id(t);

    if (!is_macro_id(t)) {
        nasm_nonfatal("`%s' expects a macro identifier", dname);
        return NULL;
    }

    id = tok_text(t);
    nasm_assert(!tok_white(t)); /* Had skip_white() here?? */
    *tp = t;
    return id;
}

/* Parse a %use package name and find the package. Set *err on syntax error. */
static const struct use_package *
get_use_pkg(Token *t, const char *dname, const char **name)
{
    const char *id;

    t = skip_white(t);
    t = expand_smacro(t);

    *name = NULL;

    if (!t) {
        nasm_nonfatal("`%s' expects a package name, got end of line", dname);
        return NULL;
    } else if (t->type != TOKEN_ID && t->type != TOKEN_STR) {
        nasm_nonfatal("`%s' expects a package name, got `%s'",
                      dname, tok_text(t));
        return NULL;
    }

    *name = id = unquote_token(t);

    t = t->next;
    t = skip_white(t);
    if (t) {
        nasm_warn(WARN_PP_TRAILING,
                  "trailing garbage after `%s' ignored", dname);
    }

    return nasm_find_use_package(id);
}

/*
 * Mark parameter tokens in an smacro definition. If the type argument
 * is 0, create smac param tokens, otherwise use the type specified;
 * normally this is used for TOKEN_XDEF_PARAM, which is used to protect
 * parameter tokens during expansion during %xdefine.
 *
 * tmpl may not be NULL here.
 */
static void mark_smac_params(Token *tline, const SMacro *tmpl,
                             enum token_type type)
{
    const struct smac_param *params = tmpl->params;
    int nparam = tmpl->nparam;
    Token *t;
    int i;

    list_for_each(t, tline) {
        if (t->type != TOKEN_ID && t->type != TOKEN_XDEF_PARAM)
            continue;

        for (i = 0; i < nparam; i++) {
            if (tok_text_match(t, &params[i].name))
                t->type = type ? type : tok_smac_param(i);
        }
    }
}

/**
 * %clear selected macro sets either globally or in contexts
 */
static void do_clear(enum clear_what what, bool context)
{
    if (context) {
        if (what & CLEAR_ALLDEFINE) {
            Context *ctx;
            list_for_each(ctx, cstk)
                clear_smacro_table(&ctx->localmac, what);
        }
        /* Nothing else can be context-local */
    } else {
        if (what & CLEAR_ALLDEFINE)
            clear_smacro_table(&smacros, what);
        if (what & CLEAR_MMACRO)
            free_mmacro_table(&mmacros);
    }
}

/*
 * Process a %line directive, including the gcc/cpp compatibility
 * form with a # at the front.
 */
static int line_directive(Token *origline, Token *tline)
{
    int k, m;
    bool err;
    const char *dname;

    /*
     * Valid syntaxes:
     * %line nnn[+mmm] [filename]
     * %line nnn[+mmm] "filename" flags...
     *
     * "flags" are for gcc compatibility and are currently ignored.
     *
     * "#" at the beginning of the line is also treated as a %line
     * directive, again for compatibility with gcc.
     */
    if ((ppopt & PP_NOLINE) || istk->mstk.mstk)
        goto done;

    dname = tok_text(tline);
    tline = tline->next;
    tline = skip_white(tline);
    if (!tok_is(tline, TOKEN_NUM)) {
        nasm_nonfatal("`%s' expects a line number", dname);
        goto done;
    }
    k = readnum(tok_text(tline), &err);
    m = 1;
    tline = tline->next;
    if (tok_is(tline, '+') || tok_is(tline, '-')) {
        bool minus = tok_is(tline, '-');
        tline = tline->next;
        if (!tok_is(tline, TOKEN_NUM)) {
            nasm_nonfatal("`%s' expects a line increment", dname);
            goto done;
        }
        m = readnum(tok_text(tline), &err);
        if (minus)
            m = -m;
        tline = tline->next;
    }
    tline = skip_white(tline);
    if (tline) {
        if (tline->type == TOKEN_STR) {
            const char *fname;
            /*
             * If this is a quoted string, ignore anything after
             * it; this allows for compatibility with gcc's
             * additional flags options.
             */

            fname = unquote_token_anystr(tline, BADCTL,
                                          dname[0] == '#' ? STR_C : STR_NASM);
            src_set_fname(fname);
        } else {
            char *fname;
            fname = detoken(tline, false);
            src_set_fname(fname);
            nasm_free(fname);
        }
    }
    src_set_linnum(k);

    istk->where = src_where();
    istk->lineinc = m;
    goto done;

done:
    free_tlist(origline);
    return DIRECTIVE_FOUND;
}

/*
 * Used for the %arg and %local directives
 */
static void define_stack_smacro(const char *name, int offset)
{
    Token *tt;

    tt = make_tok_char(NULL, ')');
    tt = make_tok_num(tt, offset);
    if (!tok_is(tt, '-'))
        tt = make_tok_char(tt, '+');
    tt = new_Token(tt, TOKEN_ID, StackPointer, 0);
    tt = make_tok_char(tt, '(');

    define_smacro(name, true, tt, NULL);
}


/*
 * This implements the %assign directive: expand an smacro expression,
 * then evaluate it, and assign the corresponding number to an smacro.
 */
static void assign_smacro(const char *mname, bool casesense,
                          Token *tline, const char *dname)
{
    struct ppscan pps;
    expr *evalresult;
    struct tokenval tokval;

    tline = expand_smacro(tline);

    pps.tptr = tline;
    pps.ntokens = -1;
    tokval.t_type = TOKEN_INVALID;
    evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
    free_tlist(tline);
    if (!evalresult)
        return;

    if (tokval.t_type) {
        nasm_warn(WARN_PP_TRAILING,
                  "trailing garbage after expression ignored");
    }
    if (!is_simple(evalresult)) {
        nasm_nonfatal("non-constant value given to `%s'", dname);
    } else {
	tline = make_tok_num(NULL, reloc_value(evalresult));

        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a numeric token to use as an expansion. Create
         * and store an SMacro.
         */
        define_smacro(mname, casesense, tline, NULL);
    }
}

/*
 * Implement string concatenation as used by the %strcat directive
 * and function.
 */
static Token *pp_strcat(Token *tline, const char *dname)
{

    size_t len;
    Token *t;
    Token *res = NULL;
    char *q, *qbuf;

    len = 0;
    list_for_each(t, tline) {
        switch (t->type) {
        case TOKEN_WHITESPACE:
        case TOKEN_COMMA:
            break;
        case TOKEN_STR:
            unquote_token(t);
            /* fall through */
        case TOKEN_INTERNAL_STR:
            len += t->len;
            break;
        default:
            nasm_nonfatal("non-string passed to `%s': %s", dname,
                          tok_text(t));
            goto err;
        }
    }

    q = qbuf = nasm_malloc(len+1);
    list_for_each(t, tline) {
        if (t->type == TOKEN_INTERNAL_STR)
            q = mempcpy(q, tok_text(t), t->len);
    }
    *q = '\0';

    res = make_tok_qstr_len(NULL, qbuf, len);
    nasm_free(qbuf);
err:
    free_tlist(tline);
    return res;
}


/*
 * Implement substring extraction as used by the %substr directive
 * and function.
 */
static Token *pp_substr_common(Token *t, int64_t start, int64_t count);

static Token *pp_substr(Token *tline, const char *dname)
{
    int64_t start, count;
    struct ppscan pps;
    Token *t;
    Token *res = NULL;
    expr *evalresult;
    struct tokenval tokval;

    t = skip_white(tline);

    if (!tok_is(t, TOKEN_STR)) {
        nasm_nonfatal("`%s' requires a string as parameter", dname);
        goto err;
    }

    pps.tptr = skip_white(t->next);
    if (tok_is(pps.tptr, TOKEN_COMMA))
        pps.tptr = skip_white(pps.tptr->next);
    if (!pps.tptr) {
        nasm_nonfatal("`%s' requires a starting index", dname);
        goto err;
    }

    pps.ntokens = -1;
    tokval.t_type = TOKEN_INVALID;
    evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
    if (!evalresult) {
        goto err;
    } else if (!is_simple(evalresult)) {
        nasm_nonfatal("non-constant value given to `%s'", dname);
        goto err;
    }
    start = evalresult->value;

    pps.tptr = skip_white(pps.tptr);
    if (!pps.tptr) {
        count = 1;  /* Backwards compatibility: one character */
    } else {
        tokval.t_type = TOKEN_INVALID;
        evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
        if (!evalresult) {
            goto err;
        } else if (!is_simple(evalresult)) {
            nasm_nonfatal("non-constant value given to `%s'", dname);
            goto err;
        }
        count = evalresult->value;
    }

    res = pp_substr_common(t, start, count);

err:
    free_tlist(tline);
    return res;
}

static Token *pp_substr_common(Token *t, int64_t start, int64_t count)
{
    size_t len;
    const char *txt;

    unquote_token(t);
    len = t->len;

    /* make start and count being in range */
    start -= 1;                 /* First character is 1 */

    if (start < 0)
        start = 0;
    if (count < 0)
        count = len + count + 1 - start;
    if (start + count > (int64_t)len)
        count = len - start;
    if (!len || count < 0 || start >=(int64_t)len)
        start = -1, count = 0; /* empty string */

    txt = (start < 0) ? "" : tok_text(t) + start;
    return make_tok_qstr_len(NULL, txt, count);
}

/**
 * find and process preprocessor directive in passed line
 * Find out if a line contains a preprocessor directive, and deal
 * with it if so.
 *
 * If a directive _is_ found, it is the responsibility of this routine
 * (and not the caller) to free_tlist() the line.
 *
 * @param tline a pointer to the current tokeninzed line linked list
 * @param output if this directive generated output
 * @return DIRECTIVE_FOUND or NO_DIRECTIVE_FOUND
 *
 */
static int do_directive(Token *tline, Token **output)
{
    enum preproc_token op;
    int j;
    enum nolist_flags nolist;
    bool casesense;
    int offset;
    const char *p;
    char *q;
    const char *found_path;
    const char *mname;
    struct ppscan pps;
    Include *inc;
    Context *ctx;
    Cond *cond;
    MMacro *mmac, **mmhead;
    Token *t = NULL, *tt, *macro_start, *last, *origline;
    Line *l;
    struct tokenval tokval;
    expr *evalresult;
    int64_t count;
    errflags severity;
    const char *dname;          /* Name of directive, for messages */

    *output = NULL;             /* No output generated */
    origline = tline;

    /* cpp-like line directive, must not be preceded by whitespace */
    if (tok_is(tline, '#'))
        return line_directive(origline, tline);

    tline = skip_white(tline);
    if (!tline)
        return NO_DIRECTIVE_FOUND;

    switch (tline->type) {
    case TOKEN_PREPROC_ID:
        dname = tok_text(tline);
        /*
         * For it to be a directive, the second character has to be an
         * ASCII letter; this is a very quick and dirty test for that;
         * all other cases will get rejected by the token hash.
         */
        if ((uint8_t)(dname[1] - 'A') > (uint8_t)('z' - 'A'))
            return NO_DIRECTIVE_FOUND;

        op = pp_token_hash(dname);
        break;

    case TOKEN_ID:
        if (likely(!(ppopt & PP_TASM)))
            return NO_DIRECTIVE_FOUND;

        dname = tok_text(tline);
        op = pp_tasm_token_hash(dname);
        break;

    default:
        return NO_DIRECTIVE_FOUND;
    }

    switch (op) {
    case PP_invalid:
        return NO_DIRECTIVE_FOUND;

    case PP_LINE:
        /*
         * %line directives are always processed immediately and
         * unconditionally, as they are intended to reflect position
         * in externally preprocessed sources.
         */
        return line_directive(origline, tline);

    default:
        break;
    }

    if (unlikely(ppopt & PP_TRIVIAL))
        goto done;

    casesense = true;
    if (PP_HAS_CASE(op) & PP_INSENSITIVE(op)) {
        casesense = false;
        op--;
    }

    /*
     * If we're in a non-emitting branch of a condition construct,
     * or walking to the end of an already terminated %rep block,
     * we should ignore all directives except for condition
     * directives.
     */
    if (((istk->conds && !emitting(istk->conds->state)) ||
         (istk->mstk.mstk && !istk->mstk.mstk->in_progress)) &&
        !is_condition(op)) {
        return NO_DIRECTIVE_FOUND;
    }

    /*
     * If we're defining a macro or reading a %rep block, we should
     * ignore all directives except for %macro/%imacro (which nest),
     * %endm/%endmacro, %line and (only if we're in a %rep block) %endrep.
     * If we're in a %rep block, another %rep nests, so should be let through.
     */
    if (defining && op != PP_MACRO && op != PP_RMACRO &&
        op != PP_ENDMACRO && op != PP_ENDM &&
        (defining->name || (op != PP_ENDREP && op != PP_REP))) {
        return NO_DIRECTIVE_FOUND;
    }

    if (defining) {
        if (op == PP_MACRO || op == PP_RMACRO) {
            nested_mac_count++;
            return NO_DIRECTIVE_FOUND;
        } else if (nested_mac_count > 0) {
            if (op == PP_ENDMACRO) {
                nested_mac_count--;
                return NO_DIRECTIVE_FOUND;
            }
        }
        if (!defining->name) {
            if (op == PP_REP) {
                nested_rep_count++;
                return NO_DIRECTIVE_FOUND;
            } else if (nested_rep_count > 0) {
                if (op == PP_ENDREP) {
                    nested_rep_count--;
                    return NO_DIRECTIVE_FOUND;
                }
            }
        }
    }

    if (pp_op_may_be_function[op]) {
        if (tok_is(skip_white(tline->next), '('))
            return NO_DIRECTIVE_FOUND; /* Expand as a preprocessor function */
    }

    switch (op) {
    default:
        nasm_nonfatal("unknown preprocessor directive `%s'", dname);
        return NO_DIRECTIVE_FOUND;      /* didn't get it */

    case PP_PRAGMA:
        /*
         * %pragma namespace options...
         *
         * The namespace "preproc" is reserved for the preprocessor;
         * all other namespaces generate a [pragma] assembly directive.
         *
         * Invalid %pragmas are ignored and may have different
         * meaning in future versions of NASM.
         */
        t = tline;
        tline = tline->next;
        t->next = NULL;
        tline = zap_white(expand_smacro(tline));
        if (tok_is(tline, TOKEN_ID)) {
            if (!nasm_stricmp(tok_text(tline), "preproc")) {
                /* Preprocessor pragma */
                do_pragma_preproc(tline);
                free_tlist(tline);
            } else {
                /* Build the assembler directive */

                /* Append bracket to the end of the output */
                for (t = tline; t->next; t = t->next)
                    ;
                t->next = make_tok_char(NULL, ']');

                /* Prepend "[pragma " */
                t = new_White(tline);
                t = new_Token(t, TOKEN_ID, "pragma", 6);
                t = make_tok_char(t, '[');
                tline = t;
                *output = tline;
            }
        }
        break;

    case PP_STACKSIZE:
    {
        const char *arg;

        /* Directive to tell NASM what the default stack size is. The
         * default is for a 16-bit stack, and this can be overridden with
         * %stacksize large.
         */
        tline = skip_white(tline->next);
        if (!tline || tline->type != TOKEN_ID) {
            nasm_nonfatal("`%s' missing size parameter", dname);
            break;
        }

        arg = tok_text(tline);

        if (nasm_stricmp(arg, "flat") == 0) {
            /* All subsequent ARG directives are for a 32-bit stack */
            StackSize = 4;
            StackPointer = "ebp";
            ArgOffset = 8;
            LocalOffset = 0;
        } else if (nasm_stricmp(arg, "flat64") == 0) {
            /* All subsequent ARG directives are for a 64-bit stack */
            StackSize = 8;
            StackPointer = "rbp";
            ArgOffset = 16;
            LocalOffset = 0;
        } else if (nasm_stricmp(arg, "large") == 0) {
            /* All subsequent ARG directives are for a 16-bit stack,
             * far function call.
             */
            StackSize = 2;
            StackPointer = "bp";
            ArgOffset = 4;
            LocalOffset = 0;
        } else if (nasm_stricmp(arg, "small") == 0) {
            /* All subsequent ARG directives are for a 16-bit stack,
             * far function call. We don't support near functions.
             */
            StackSize = 2;
            StackPointer = "bp";
            ArgOffset = 6;
            LocalOffset = 0;
        } else {
            nasm_nonfatal("`%s' invalid size type", dname);
        }
        break;
    }

    case PP_ARG:
        /* TASM like ARG directive to define arguments to functions, in
         * the following form:
         *
         *      ARG arg1:WORD, arg2:DWORD, arg4:QWORD
         */
        offset = ArgOffset;
        do {
            const char *arg;
            int size = StackSize;

            /* Find the argument name */
            tline = skip_white(tline->next);
            if (!tline || tline->type != TOKEN_ID) {
                nasm_nonfatal("`%s' missing argument parameter", dname);
                goto done;
            }
            arg = tok_text(tline);

            /* Find the argument size type */
            tline = tline->next;
            if (!tok_is(tline, ':')) {
                nasm_nonfatal("syntax error processing `%s' directive", dname);
                goto done;
            }
            tline = tline->next;
            if (!tok_is(tline, TOKEN_ID)) {
                nasm_nonfatal("`%s' missing size type parameter", dname);
                goto done;
            }

            /* Allow macro expansion of type parameter */
            tt = tokenize(tok_text(tline));
            tt = expand_smacro(tt);
            size = parse_size(tok_text(tt));
            if (!size) {
                nasm_nonfatal("invalid size type for `%s' missing directive", dname);
                free_tlist(tt);
                goto done;
            }
            free_tlist(tt);

            /* Round up to even stack slots */
            size = ALIGN(size, StackSize);

            /* Now define the macro for the argument */
            define_stack_smacro(arg, offset);
            offset += size;

            /* Move to the next argument in the list */
            tline = skip_white(tline->next);
        } while (tok_is(tline, ','));
        ArgOffset = offset;
        break;

    case PP_LOCAL:
    {
        int total_size = 0;

        /* TASM like LOCAL directive to define local variables for a
         * function, in the following form:
         *
         *      LOCAL local1:WORD, local2:DWORD, local4:QWORD = LocalSize
         *
         * The '= LocalSize' at the end is ignored by NASM, but is
         * required by TASM to define the local parameter size (and used
         * by the TASM macro package).
         */
        offset = LocalOffset;
        do {
            const char *local;
            int size = StackSize;

            /* Find the argument name */
            tline = skip_white(tline->next);
            if (!tline || tline->type != TOKEN_ID) {
                nasm_nonfatal("`%s' missing argument parameter", dname);
                goto done;
            }
            local = tok_text(tline);

            /* Find the argument size type */
            tline = tline->next;
            if (!tok_is(tline, ':')) {
                nasm_nonfatal("syntax error processing `%s' directive", dname);
                goto done;
            }
            tline = tline->next;
            if (!tok_is(tline, TOKEN_ID)) {
                nasm_nonfatal("`%s' missing size type parameter", dname);
                goto done;
            }

            /* Allow macro expansion of type parameter */
            tt = tokenize(tok_text(tline));
            tt = expand_smacro(tt);
            size = parse_size(tok_text(tt));
            if (!size) {
                nasm_nonfatal("invalid size type for `%s' missing directive", dname);
                free_tlist(tt);
                goto done;
            }
            free_tlist(tt);

            /* Round up to even stack slots */
            size = ALIGN(size, StackSize);

            offset += size;     /* Negative offset, increment before */

            /* Now define the macro for the argument */
            define_stack_smacro(local, -offset);

            /* How is this different from offset? */
            total_size += size;

            /* Move to the next argument in the list */
            tline = skip_white(tline->next);
        } while (tok_is(tline, ','));

        /* Now define the assign to setup the enter_c macro correctly */
        tt = make_tok_num(NULL, total_size);
        tt = make_tok_char(tt, '+');
        tt = new_Token(tt, TOKEN_LOCAL_MACRO, "%$localsize", 11);
        assign_smacro("%$localsize", true, tt, dname);

        LocalOffset = offset;
        break;
    }
    case PP_CLEAR:
    {
        bool context = false;

        t = tline->next = expand_smacro(tline->next);
        t = skip_white(t);
        if (!t) {
            /* Emulate legacy behavior */
            do_clear(CLEAR_DEFINE|CLEAR_MMACRO, false);
        } else {
            while ((t = skip_white(t)) && t->type == TOKEN_ID) {
                const char *txt = tok_text(t);
                if (!nasm_stricmp(txt, "all")) {
                    do_clear(CLEAR_ALL, context);
                } else if (!nasm_stricmp(txt, "define") ||
                           !nasm_stricmp(txt, "def") ||
                           !nasm_stricmp(txt, "smacro")) {
                    do_clear(CLEAR_DEFINE, context);
                } else if (!nasm_stricmp(txt, "defalias") ||
                           !nasm_stricmp(txt, "alias") ||
                           !nasm_stricmp(txt, "salias")) {
                    do_clear(CLEAR_DEFALIAS, context);
                } else if (!nasm_stricmp(txt, "alldef") ||
                           !nasm_stricmp(txt, "alldefine")) {
                    do_clear(CLEAR_ALLDEFINE, context);
                } else if (!nasm_stricmp(txt, "macro") ||
                           !nasm_stricmp(txt, "mmacro")) {
                    do_clear(CLEAR_MMACRO, context);
                } else if (!nasm_stricmp(txt, "context") ||
                           !nasm_stricmp(txt, "ctx")) {
                    context = true;
                } else if (!nasm_stricmp(txt, "global")) {
                    context = false;
                } else if (!nasm_stricmp(txt, "nothing") ||
                         !nasm_stricmp(txt, "none") ||
                         !nasm_stricmp(txt, "ignore") ||
                         !nasm_stricmp(txt, "-") ||
                         !nasm_stricmp(txt, "--")) {
                    /* Do nothing */
                } else {
                    nasm_nonfatal("invalid option to %s: %s", dname, txt);
                    t = NULL;
                }
            }
        }

        t = skip_white(t);
        if (t) {
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after `%s' ignored", dname);
        }
        break;
    }

    case PP_DEPEND:
        t = tline->next = expand_smacro(tline->next);
        t = skip_white(t);
        if (!t || (t->type != TOKEN_STR &&
                   t->type != TOKEN_INTERNAL_STR)) {
            nasm_nonfatal("`%s' expects a file name", dname);
            goto done;
        }
        if (skip_white(t->next)) {
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after `%s' ignored", dname);
        }

        strlist_add(deplist, unquote_token_cstr(t));
        goto done;

    case PP_INCLUDE:
    case PP_REQUIRE:
        t = tline->next = expand_smacro(tline->next);
        t = skip_white(t);

        if (!t || (t->type != TOKEN_STR &&
                   t->type != TOKEN_INTERNAL_STR)) {
            nasm_nonfatal("`%s' expects a file name", dname);
            goto done;
        }
        if (skip_white(t->next)) {
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after `%s' ignored", dname);
        }
        p = unquote_token_cstr(t);
        nasm_new(inc);
        inc->next = istk;
        found_path = NULL;
        inc->fp = inc_fopen(p, deplist, &found_path,
                            (pp_mode == PP_DEPS) ? INC_OPTIONAL :
                            (op == PP_REQUIRE) ? INC_REQUIRED :
                            INC_NEEDED, NF_TEXT);
        if (!inc->fp) {
            /* -MG given but file not found, or repeated %require */
            nasm_free(inc);
        } else {
            inc->nolist  = istk->nolist;
            inc->noline  = istk->noline;
            inc->where   = istk->where;
            inc->lineinc = 0;
            istk = inc;
            if (!istk->noline) {
                src_set(0, found_path ? found_path : p);
                istk->where = src_where();
                istk->lineinc = 1;
                if (ppdbg & PDBG_INCLUDE)
                    dfmt->debug_include(true, istk->next->where, istk->where);
            }
            if (!istk->nolist)
                lfmt->uplevel(LIST_INCLUDE, 0);
        }
        break;

    case PP_USE:
    {
        const struct use_package *pkg;
        const char *name;

        pkg = get_use_pkg(tline->next, dname, &name);
        if (!name)
            goto done;
        if (!pkg) {
            nasm_nonfatal("unknown `%s' package: `%s'", dname, name);
        } else if (!use_loaded[pkg->index]) {
            /*
             * Not already included, go ahead and include it.
             * Treat it as an include file for the purpose of
             * producing a listing.
             */
            use_loaded[pkg->index] = true;
            stdmacpos = pkg->macros;
            nasm_new(inc);
            inc->next = istk;
            if (!list_option('b')) {
                inc->nolist++;
                inc->noline++;
            }
            istk = inc;
            if (!istk->nolist)
                lfmt->uplevel(LIST_INCLUDE, 0);
            if (!inc->noline)
                src_set(0, NULL);
        }
        break;
    }
    case PP_PUSH:
    case PP_REPL:
    case PP_POP:
        tline = tline->next;
        tline = skip_white(tline);
        tline = expand_id(tline);
        if (tline) {
            if (!tok_is(tline, TOKEN_ID)) {
                nasm_nonfatal("`%s' expects a context identifier", dname);
                goto done;
            }
            if (skip_white(tline->next)) {
                nasm_warn(WARN_PP_TRAILING, "trailing garbage after `%s' ignored",
                           dname);
            }
            p = tok_text(tline);
        } else {
            p = NULL; /* Anonymous */
        }

        if (op == PP_PUSH) {
            nasm_new(ctx);
            ctx->depth = cstk ? cstk->depth + 1 : 1;
            ctx->next = cstk;
            ctx->name = p ? nasm_strdup(p) : NULL;
            ctx->number = unique++;
            cstk = ctx;
        } else {
            /* %pop or %repl */
            if (!cstk) {
                nasm_nonfatal("`%s': context stack is empty", dname);
            } else if (op == PP_POP) {
                if (p && (!cstk->name || nasm_stricmp(p, cstk->name)))
                    nasm_nonfatal("`%s' in wrong context: %s, "
                               "expected %s",
                               dname, cstk->name ? cstk->name : "anonymous", p);
                else
                    ctx_pop();
            } else {
                /* op == PP_REPL */
                nasm_free((char *)cstk->name);
                cstk->name = p ? nasm_strdup(p) : NULL;
                p = NULL;
            }
        }
        break;
    case PP_FATAL:
        severity = ERR_FATAL;
        goto issue_error;
    case PP_ERROR:
        severity = ERR_NONFATAL|ERR_PASS2;
        goto issue_error;
    case PP_WARNING:
        /*!
         *!user [on] \c{%warning} directives
         *!  controls output of \c{%warning} directives (see \k{pperror}).
         */
        severity = ERR_WARNING|WARN_USER|ERR_PASS2;
        goto issue_error;
    case PP_NOTE:
        severity = ERR_NOTE;
        goto issue_error;

issue_error:
    {
        /* Only error out if this is the final pass */
        tline->next = expand_smacro(tline->next);
        tline = tline->next;
        tline = skip_white(tline);
        t = tline ? tline->next : NULL;
        t = skip_white(t);
        if (tok_is(tline, TOKEN_STR) && !t) {
            /* The line contains only a quoted string */
            p = unquote_token(tline); /* Ignore NUL character truncation */
            nasm_error(severity, "%s",  p);
        } else {
            /* Not a quoted string, or more than a quoted string */
            q = detoken(tline, false);
            nasm_error(severity, "%s",  q);
            nasm_free(q);
        }
        break;
    }

    CASE_PP_IF:
        if (istk->conds && !emitting(istk->conds->state))
            j = COND_NEVER;
        else {
            j = if_condition(tline->next, op);
            tline->next = NULL; /* it got freed */
        }
        cond = nasm_malloc(sizeof(Cond));
        cond->next = istk->conds;
        cond->state = j;
        istk->conds = cond;
        if(istk->mstk.mstk)
            istk->mstk.mstk->condcnt++;
        break;

    CASE_PP_ELIF:
        if (!istk->conds) {
            nasm_nonfatal("`%s': no matching `%%if'", dname);
            break;
        }
        switch(istk->conds->state) {
        case COND_IF_TRUE:
            istk->conds->state = COND_DONE;
            break;

        case COND_DONE:
        case COND_NEVER:
            break;

        case COND_ELSE_TRUE:
        case COND_ELSE_FALSE:
            /*!
             *!pp-else-elif [on] \c{%elif} after \c{%else}
             *!  warns that an \c{%elif}-type directive was encountered
             *!  after \c{%else} has already been encounted. As a result, the
             *!  content of the \c{%elif} will never be expanded.
             */
            nasm_warn(WARN_PP_ELSE_ELIF|ERR_PP_PRECOND,
                       "`%s' after `%%else', ignoring content", dname);
            istk->conds->state = COND_NEVER;
            break;

        case COND_IF_FALSE:
            /*
             * IMPORTANT: In the case of %if, we will already have
             * called expand_mmac_params(); however, if we're
             * processing an %elif we must have been in a
             * non-emitting mode, which would have inhibited
             * the normal invocation of expand_mmac_params().
             * Therefore, we have to do it explicitly here.
             */
            j = if_condition(expand_mmac_params(tline->next), op);
            tline->next = NULL; /* it got freed */
            istk->conds->state = j;
            break;
        }
        break;

    case PP_ELSE:
        if (tline->next)
            nasm_warn(WARN_PP_TRAILING|ERR_PP_PRECOND,
                      "trailing garbage after `%s' ignored", dname);
        if (!istk->conds) {
	    nasm_nonfatal("`%s': no matching `%%if'", dname);
            break;
        }
        switch(istk->conds->state) {
        case COND_IF_TRUE:
        case COND_DONE:
            istk->conds->state = COND_ELSE_FALSE;
            break;

        case COND_NEVER:
            break;

        case COND_IF_FALSE:
            istk->conds->state = COND_ELSE_TRUE;
            break;

        case COND_ELSE_TRUE:
        case COND_ELSE_FALSE:
            /*!
             *!pp-else-else [on] \c{%else} after \c{%else}
             *!  warns that a second \c{%else} clause was found for
             *!  the same \c{%if} statement. The content of this \c{%else}
             *!  clause will never be expanded.
             */
            nasm_warn(WARN_PP_ELSE_ELSE|ERR_PP_PRECOND,
                      "`%s' after `%%else', ignoring content", dname);
            istk->conds->state = COND_NEVER;
            break;
        }
        break;

    case PP_ENDIF:
        if (tline->next) {
            nasm_warn(WARN_PP_TRAILING|ERR_PP_PRECOND,
                      "trailing garbage after `%s' ignored", dname);
        }
        if (!istk->conds) {
            nasm_nonfatal("`%s': no matching `%%if'", dname);
            break;
        }
        cond = istk->conds;
        istk->conds = cond->next;
        nasm_free(cond);
        if(istk->mstk.mstk)
            istk->mstk.mstk->condcnt--;
        break;

    case PP_RMACRO:
    case PP_MACRO:
    {
        MMacro *def;

        nasm_assert(!defining);
        nasm_new(def);
        def->casesense = casesense;
        /*
         * dstk.mstk points to the previous definition bracket,
         * whereas dstk.mmac points to the topmost mmacro, which
         * in this case is the one we are just starting to create.
         */
        def->dstk.mstk = defining;
        def->dstk.mmac = def;
        if (op == PP_RMACRO)
            def->max_depth = nasm_limit[LIMIT_MACRO_LEVELS];
        if (!parse_mmacro_spec(tline, def, dname)) {
            nasm_free(def);
            goto done;
        }

        defining = def;
        defining->where = istk->where;

        mmac = (MMacro *) hash_findix(&mmacros, defining->name);
        while (mmac) {
            if (!strcmp(mmac->name, defining->name) &&
                (mmac->nparam_min <= defining->nparam_max
                 || defining->plus)
                && (defining->nparam_min <= mmac->nparam_max
                    || mmac->plus)) {
                /*!
                 *!pp-macro-redef-multi [on] redefining multi-line macro
                 *!  warns that a multi-line macro is being redefined,
                 *!  without first removing the old definition with
                 *!  \c{%unmacro}.
                 */
                nasm_warn(WARN_PP_MACRO_REDEF_MULTI,
                          "redefining multi-line macro `%s'",
                           defining->name);
                break;
            }
            mmac = mmac->next;
        }
        break;
    }

    case PP_ENDM:
    case PP_ENDMACRO:
        if (!(defining && defining->name)) {
            nasm_nonfatal("`%s': not defining a macro", tok_text(tline));
            goto done;
        }
        mmhead = (MMacro **) hash_findi_add(&mmacros, defining->name);
        defining->next = *mmhead;
        *mmhead = defining;
        defining = NULL;
        break;

    case PP_EXITMACRO:
        /*
         * We must search along istk->expansion until we hit a
         * macro-end marker for a macro with a name. Then we
         * bypass all lines between exitmacro and endmacro.
         */
        list_for_each(l, istk->expansion)
            if (l->finishes && l->finishes->name)
                break;

        if (l) {
            /*
             * Remove all conditional entries relative to this
             * macro invocation. (safe to do in this context)
             */
            for ( ; l->finishes->condcnt > 0; l->finishes->condcnt --) {
                cond = istk->conds;
                if (!cond) {
                    l->finishes->condcnt = 0;
                    break;      /* Possible in case of invalid nesting */
                }
                istk->conds = cond->next;
                nasm_free(cond);
            }
            istk->expansion = l;
        } else {
            nasm_nonfatal("`%%exitmacro' not within `%%macro' block");
        }
        break;

    case PP_UNIMACRO:
        casesense = false;
        /* fall through */
    case PP_UNMACRO:
    {
        MMacro **mmac_p;
        MMacro spec;

        nasm_zero(spec);
        spec.casesense = casesense;
        if (!parse_mmacro_spec(tline, &spec, dname)) {
            goto done;
        }
        mmac_p = (MMacro **) hash_findi(&mmacros, spec.name, NULL);
        if (!mmac_p) {
            /* No such macro */
            free_tlist(spec.dlist);
            break;
        }

        /* Check the macro to be undefined is not being expanded */
        list_for_each(l, istk->expansion) {
            if (l->finishes == *mmac_p) {
                nasm_nonfatal("`%%unmacro' can't undefine the macro being expanded");
                /*
                 * Do not release the macro instance to avoid using the freed
                 * memory while proceeding the expansion.
                 */
                goto done;
            }
        }

        while (mmac_p && *mmac_p) {
            mmac = *mmac_p;
            if (mmac->casesense == spec.casesense &&
                !mstrcmp(mmac->name, spec.name, spec.casesense) &&
                mmac->nparam_min == spec.nparam_min &&
                mmac->nparam_max == spec.nparam_max &&
                mmac->plus == spec.plus) {
                *mmac_p = mmac->next;
                free_mmacro(mmac);
            } else {
                mmac_p = &mmac->next;
            }
        }
        free_tlist(spec.dlist);
        break;
    }

    case PP_ROTATE:
        while (tok_white(tline->next))
            tline = tline->next;
        if (!tline->next) {
            free_tlist(origline);
            nasm_nonfatal("`%s' missing rotate count", dname);
            return DIRECTIVE_FOUND;
        }
        t = expand_smacro(tline->next);
        tline->next = NULL;
        pps.tptr = tline = t;
	pps.ntokens = -1;
        tokval.t_type = TOKEN_INVALID;
        evalresult =
            evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
        free_tlist(tline);
        if (!evalresult)
            return DIRECTIVE_FOUND;
        if (tokval.t_type) {
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after expression ignored");
        }
        if (!is_simple(evalresult)) {
            nasm_nonfatal("non-constant value given to `%s'", dname);
            return DIRECTIVE_FOUND;
        }
        mmac = istk->mstk.mmac;
        if (!mmac) {
            nasm_nonfatal("`%s' invoked outside a macro call", dname);;
        } else if (mmac->nparam == 0) {
            nasm_nonfatal("`%s' invoked within macro without parameters", dname);
        } else {
            int rotate = mmac->rotate + reloc_value(evalresult);

            rotate %= (int)mmac->nparam;
            if (rotate < 0)
                rotate += mmac->nparam;

            mmac->rotate = rotate;
        }
        break;

    case PP_REP:
    {
        MMacro *tmp_defining;

        nolist = 0;
        tline = skip_white(tline->next);
        if (tok_is(tline, TOKEN_ID) && tline->len == 7 &&
	    !nasm_memicmp(tline->text.a, ".nolist", 7)) {
            if (!list_option('f'))
                nolist |= NL_LIST; /* ... but update line numbers */
            tline = skip_white(tline->next);
        }

        if (tline) {
            pps.tptr = expand_smacro(tline);
	    pps.ntokens = -1;
            tokval.t_type = TOKEN_INVALID;
            /* XXX: really critical?! */
            evalresult =
                evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
            if (!evalresult)
                goto done;
            if (tokval.t_type)
                nasm_warn(WARN_PP_TRAILING, "trailing garbage after expression ignored");
            if (!is_simple(evalresult)) {
                nasm_nonfatal("non-constant value given to `%s'", dname);
                goto done;
            }
            count = reloc_value(evalresult);
            if (count > nasm_limit[LIMIT_REP]) {
                nasm_nonfatal("`%s' count %"PRId64" exceeds limit (currently %"PRId64")",
                              dname, count, nasm_limit[LIMIT_REP]);
                count = 0;
            } else if (count < 0) {
                /*!
                 *!pp-rep-negative [on] regative \c{%rep} count
                 *!=negative-rep
                 *!  warns about a negative count given to the \c{%rep}
                 *!  preprocessor directive.
                 */
                nasm_warn(ERR_PASS2|WARN_PP_REP_NEGATIVE,
                          "negative `%s' count: %"PRId64, dname, count);
                count = 0;
            } else {
                count++;
            }
        } else {
            nasm_nonfatal("`%s' expects a repeat count", dname);
            count = 0;
        }
        tmp_defining = defining;
        nasm_new(defining);
        defining->nolist = nolist;
        defining->in_progress = count;
        defining->mstk = istk->mstk;
        defining->dstk.mstk = tmp_defining;
        defining->dstk.mmac = tmp_defining ? tmp_defining->dstk.mmac : NULL;
        defining->where = istk->where;
        break;
    }

    case PP_ENDREP:
        if (!defining || defining->name) {
            nasm_nonfatal("`%%endrep': no matching `%%rep'");
            goto done;
        }

        /*
         * Now we have a "macro" defined - although it has no name
         * and we won't be entering it in the hash tables - we must
         * push a macro-end marker for it on to istk->expansion.
         * After that, it will take care of propagating itself (a
         * macro-end marker line for a macro which is really a %rep
         * block will cause the macro to be re-expanded, complete
         * with another macro-end marker to ensure the process
         * continues) until the whole expansion is forcibly removed
         * from istk->expansion by a %exitrep.
         */
        nasm_new(l);
        l->next = istk->expansion;
        l->finishes = defining;
        l->first = NULL;
        l->where = src_where();
        istk->expansion = l;

        istk->mstk.mstk = defining;

        /* A loop does not change istk->noline */
        istk->nolist += !!(defining->nolist & NL_LIST);
        if (!istk->nolist)
            lfmt->uplevel(LIST_MACRO, 0);

        defining = defining->dstk.mstk;
        break;

    case PP_EXITREP:
        /*
         * We must search along istk->expansion until we hit a
         * macro-end marker for a macro with no name. Then we set
         * its `in_progress' flag to 0.
         */
        list_for_each(l, istk->expansion)
            if (l->finishes && !l->finishes->name)
                break;

        if (l)
            l->finishes->in_progress = 0;
        else
            nasm_nonfatal("`%%exitrep' not within `%%rep' block");
        break;

    case PP_DEFINE:
    case PP_XDEFINE:
    case PP_DEFALIAS:
    {
        SMacro tmpl;
        Token **lastp;
        int nparam;

        if (!(mname = get_id(&tline, dname)))
            goto done;

        nasm_zero(tmpl);
        lastp = &tline->next;
        nparam = parse_smacro_template(&lastp, &tmpl);
        tline = *lastp;
        *lastp = NULL;

        if (unlikely(op == PP_DEFALIAS)) {
            macro_start = tline;
            if (!is_macro_id(macro_start)) {
                nasm_nonfatal("`%s' expects a macro identifier to alias",
                              dname);
                goto done;
            }
            tt = macro_start->next;
            macro_start->next = NULL;
            tline = tline->next;
            tline = skip_white(tline);
            if (tline && tline->type) {
                nasm_warn(WARN_PP_TRAILING,
                          "trailing garbage after aliasing identifier ignored");
            }
            free_tlist(tt);
            tmpl.alias = true;
        } else {
            if (op == PP_XDEFINE) {
                /* Protect macro parameter tokens */
                if (nparam)
                    mark_smac_params(tline, &tmpl, TOKEN_XDEF_PARAM);
                tline = expand_smacro(tline);
            }
            macro_start = tline;
        }

        /*
         * Good. We now have a macro name, a parameter count, and a
         * token list (in reverse order) for an expansion. We ought
         * to be OK just to create an SMacro, store it, and let
         * free_tlist have the rest of the line (which we have
         * carefully re-terminated after chopping off the expansion
         * from the end).
         */
        define_smacro(mname, casesense, macro_start, &tmpl);
        break;
    }

    case PP_UNDEF:
    case PP_UNDEFALIAS:
        if (!(mname = get_id(&tline, dname)))
            goto done;
        if (tline->next)
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after macro name ignored");

        undef_smacro(mname, op == PP_UNDEFALIAS);
        break;

    case PP_DEFSTR:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        tline = zap_white(tline);
        q = detoken(tline, false);
        macro_start = make_tok_qstr(NULL, q);
        nasm_free(q);

        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a string token to use as an expansion. Create
         * and store an SMacro.
         */
        define_smacro(mname, casesense, macro_start, NULL);
        break;

    case PP_DEFTOK:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        t = skip_white(tline);
        /* t should now point to the string */
        if (!tok_is(t, TOKEN_STR)) {
            nasm_nonfatal("`%s' requires string as second parameter", dname);
            free_tlist(tline);
            goto done;
        }

        /*
         * Convert the string to a token stream.
         */
        macro_start = tokenize(unquote_token_cstr(t));

        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a numeric token to use as an expansion. Create
         * and store an SMacro.
         */
        define_smacro(mname, casesense, macro_start, NULL);
        free_tlist(tline);
        break;

    case PP_PATHSEARCH:
    {
        const char *found_path;

        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        t = skip_white(tline);
        if (!t || (t->type != TOKEN_STR &&
                   t->type != TOKEN_INTERNAL_STR)) {
            nasm_nonfatal("`%s' expects a file name", dname);
            free_tlist(tline);
            goto done;
        }
        if (t->next)
            nasm_warn(WARN_PP_TRAILING,
                      "trailing garbage after `%s' ignored", dname);

	p = unquote_token_cstr(t);

        inc_fopen(p, NULL, &found_path, INC_PROBE, NF_BINARY);
        if (!found_path)
            found_path = p;
	macro_start = make_tok_qstr(NULL, found_path);

        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a string token to use as an expansion. Create
         * and store an SMacro.
         */
        define_smacro(mname, casesense, macro_start, NULL);
        free_tlist(tline);
        break;
    }

    case PP_STRLEN:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        t = skip_white(tline);
        /* t should now point to the string */
        if (!tok_is(t, TOKEN_STR)) {
            nasm_nonfatal("`%s' requires string as second parameter", dname);
            free_tlist(tline);
            free_tlist(origline);
            return DIRECTIVE_FOUND;
        }

	unquote_token(t);
        macro_start = make_tok_num(NULL, t->len);

        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a numeric token to use as an expansion. Create
         * and store an SMacro.
         */
        define_smacro(mname, casesense, macro_start, NULL);
        free_tlist(tline);
        free_tlist(origline);
        return DIRECTIVE_FOUND;

    case PP_STRCAT:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        macro_start = pp_strcat(tline, dname);
        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a string token to use as an expansion. Create
         * and store an SMacro.
         */
        if (macro_start)
            define_smacro(mname, casesense, macro_start, NULL);
        break;

    case PP_SUBSTR:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = expand_smacro(tline->next);
        last->next = NULL;

        macro_start = pp_substr(tline, dname);
        /*
         * We now have a macro name, an implicit parameter count of
         * zero, and a string token to use as an expansion. Create
         * and store an SMacro.
         */
        if (macro_start)
            define_smacro(mname, casesense, macro_start, NULL);
        break;

    case PP_ASSIGN:
        if (!(mname = get_id(&tline, dname)))
            goto done;

        last = tline;
        tline = tline->next;
        last->next = NULL;
        assign_smacro(mname, casesense, tline, dname);
        goto done;

    case PP_ALIASES:
        tline = tline->next;
        tline = expand_smacro(tline);
        ppconf.noaliases = !pp_get_boolean_option(tline, !ppconf.noaliases);
        break;

    case PP_LINE:
        nasm_panic("`%s' directive not preprocessed early", dname);
        break;

    case PP_NULL:
        /* Goes nowhere, does nothing... */
        break;

    }

done:
    free_tlist(origline);
    return DIRECTIVE_FOUND;
}

/*
 * Ensure that a macro parameter contains a condition code and
 * nothing else. Return the condition code index if so, or -1
 * otherwise.
 */
static int find_cc(Token * t)
{
    Token *tt;

    if (!t)
        return -1;              /* Probably a %+ without a space */

    t = skip_white(t);
    if (!tok_is(t, TOKEN_ID))
        return -1;
    tt = t->next;
    tt = skip_white(tt);
    if (tok_isnt(tt, ','))
        return -1;

    return bsii(tok_text(t), (const char **)conditions,
		ARRAY_SIZE(conditions));
}

enum concat_flags {
    CONCAT_ID             = 0x01,
    CONCAT_LOCAL_MACRO    = 0x02,
    CONCAT_ENVIRON        = 0x04,
    CONCAT_PREPROC_ID     = 0x08,
    CONCAT_NUM            = 0x10,
    CONCAT_FLOAT          = 0x20,
    CONCAT_OP             = 0x40  /* Operators */
};

struct concat_mask {
    enum concat_flags mask_head;
    enum concat_flags mask_tail;
};


static inline bool pp_concat_match(const Token *t, enum concat_flags mask)
{
    enum concat_flags ctype = 0;

    if (!t)
        return false;

    switch (t->type) {
    case TOKEN_ID:
    case TOKEN_QMARK:           /* Keyword, treated as ID for pasting */
        ctype = CONCAT_ID;
        break;
    case TOKEN_LOCAL_MACRO:
        ctype = CONCAT_LOCAL_MACRO;
        break;
    case TOKEN_ENVIRON:
        ctype = CONCAT_ENVIRON;
        break;
    case TOKEN_PREPROC_ID:
        ctype = CONCAT_PREPROC_ID;
        break;
    case TOKEN_NUM:
    case TOKEN_FLOAT:
        ctype = CONCAT_NUM;
        break;
    case TOKEN_HERE:
    case TOKEN_BASE:
        /* NASM 2.15 treats these as operators, but is that sane? */
        ctype = CONCAT_OP;
        break;
    case TOKEN_OTHER:
        ctype = CONCAT_OP;      /* For historical reasons */
        break;
    default:
        if (t->type > TOKEN_WHITESPACE && t->type < TOKEN_MAX_OPERATOR)
            ctype = CONCAT_OP;
        else
            ctype = 0;
    }

    return !!(ctype & mask);
}

/*
 * This routines walks over tokens stream and handles tokens
 * pasting, if @handle_explicit passed then explicit pasting
 * term is handled, otherwise -- implicit pastings only.
 * The @m array can contain a series of token types which are
 * executed as separate passes.
 */
static bool paste_tokens(Token **head, const struct concat_mask *m,
                         size_t mnum, bool handle_explicit)
{
    Token *tok, *t, *next, **prev_next, **prev_nonspace, **nextp;
    bool pasted = false;
    char *buf, *p;
    size_t len, i;

    /*
     * The last token before pasting. We need it
     * to be able to connect new handled tokens.
     * In other words if there were a tokens stream
     *
     * A -> B -> C -> D
     *
     * and we've joined tokens B and C, the resulting
     * stream should be
     *
     * A -> BC -> D
     */
    tok = *head;
    prev_next = prev_nonspace = head;

    if (tok_white(tok) || tok_is(tok, TOKEN_PASTE))
        prev_nonspace = NULL;

    while (tok && (next = tok->next)) {
        bool did_paste = false;

        switch (tok->type) {
        case TOKEN_WHITESPACE:
            /* Zap redundant whitespaces */
            tok->next = next = zap_white(next);
            break;

        case TOKEN_PASTE:
            /* Explicit pasting */
            if (!handle_explicit)
                break;

            did_paste = true;

            /* Left pasting token is start of line, just drop %+ */
            if (!prev_nonspace) {
                prev_next = nextp = head;
                t = NULL;
            } else {
                prev_next = prev_nonspace;
                t = *prev_next;
                nextp = &t->next;
            }

            /*
             * Delete the %+ token itself plus any whitespace.
             * In a sequence of %+ ... %+ ... %+ pasting sequences where
             * some expansions in the middle have ended up empty,
             * we can end up having multiple %+ tokens in a row;
             * just drop whem in that case.
             */
            next = *nextp;
            while (next) {
                if (next->type == TOKEN_PASTE || next->type == TOKEN_WHITESPACE)
                    next = delete_Token(next);
                else
                    break;
            }
            *nextp = next;

            /*
             * Nothing after? Just leave the existing token.
             */
            if (!next)
                break;

            if (!t) {
                /* Nothing to actually paste, just zapping the paste */
                *prev_next = tok = next;
                break;
            }

            /* An actual paste */
            p = buf = nasm_malloc(t->len + next->len + 1);
            p = mempcpy(p, tok_text(t), t->len);
            p = mempcpy(p, tok_text(next), next->len);
            *p = '\0';
            delete_Token(t);
            t = tokenize(buf);
            nasm_free(buf);

            if (unlikely(!t)) {
                /*
                 * No output at all? Replace with a single whitespace.
                 * This should never happen.
                 */
                tok = t = new_White(NULL);
            } else {
                *prev_nonspace = tok = t;
            }
            while (t->next)
                t = t->next;    /* Find the last token produced */

            /* Delete the second token and attach to the end of the list */
            t->next = delete_Token(next);

            /* We want to restart from the head of the pasted token */
            *prev_next = next = tok;
            break;

        default:
            /* implicit pasting */
            for (i = 0; i < mnum; i++) {
                if (pp_concat_match(tok, m[i].mask_head))
                    break;
            }

            if (i >= mnum)
                break;

            len =  tok->len;
            while (pp_concat_match(next, m[i].mask_tail)) {
                len += next->len;
                next = next->next;
            }

            /* No match or no text to process */
            if (len == tok->len)
                break;

            p = buf = nasm_malloc(len + 1);
            while (tok != next) {
                p = mempcpy(p, tok_text(tok), tok->len);
                tok = delete_Token(tok);
            }
            *p = '\0';
            *prev_next = tok = t = tokenize(buf);
            nasm_free(buf);

            /*
             * Connect pasted into original stream,
             * ie A -> new-tokens -> B
             */
            while ((tok = t->next)) {
                if (tok->type != TOKEN_WHITESPACE && tok->type != TOKEN_PASTE)
                    prev_nonspace = &t->next;
                t = tok;
            }

            t->next = next;
            prev_next = &t->next;
            did_paste = true;
            break;
        }

        if (did_paste) {
            pasted = true;
        } else {
            prev_next = &tok->next;
            if (next && next->type != TOKEN_WHITESPACE &&
                next->type != TOKEN_PASTE)
                prev_nonspace = prev_next;
        }
        tok = next;
    }

    return pasted;
}

/*
 * Computes the proper rotation of mmacro parameters
 */
static int mmac_rotate(const MMacro *mac, unsigned int n)
{
    if (--n < mac->nparam)
        n = (n + mac->rotate) % mac->nparam;

    return n+1;
}

/*
 * expands to a list of tokens from %{x:y}
 */
static void expand_mmac_params_range(MMacro *mac, Token *tline, Token ***tail)
{
    Token *t;
    const char *arg = tok_text(tline) + 1;
    int fst, lst, incr, n;
    int parsed;

    parsed = sscanf(arg, "%d:%d", &fst, &lst);
    nasm_assert(parsed == 2);

    /*
     * only macros params are accounted so
     * if someone passes %0 -- we reject such
     * value(s)
     */
    if (lst == 0 || fst == 0)
        goto err;

    /* the values should be sane */
    if ((fst > (int)mac->nparam || fst < (-(int)mac->nparam)) ||
        (lst > (int)mac->nparam || lst < (-(int)mac->nparam)))
        goto err;

    fst = fst < 0 ? fst + (int)mac->nparam + 1: fst;
    lst = lst < 0 ? lst + (int)mac->nparam + 1: lst;

    /*
     * It will be at least one parameter, as we can loop
     * in either direction.
     */
    incr = (fst < lst) ? 1 : -1;

    while (true) {
        n = mmac_rotate(mac, fst);
        dup_tlistn(mac->params[n], mac->paramlen[n], tail);
        if (fst == lst)
            break;
        t = make_tok_char(NULL, ',');
        **tail = t;
        *tail = &t->next;
        fst += incr;
    }

    return;

err:
    nasm_nonfatal("`%%{%s}': macro parameters out of range", arg);
    return;
}

/*
 * Expand MMacro-local things: parameter references (%0, %n, %+n,
 * %-n) and MMacro-local identifiers (%%foo) as well as
 * macro indirection (%[...]) and range (%{..:..}).
 */
static Token *expand_mmac_params(Token * tline)
{
    Token **tail, *thead;
    bool changed = false;
    MMacro *mac = istk->mstk.mmac;

    tail = &thead;
    thead = NULL;

    while (tline) {
        bool change;
        bool err_not_mac = false;
        Token *t = tline;
        const char *text = tok_text(t);
        int type = t->type;

        tline = tline->next;
        t->next = NULL;

        switch (type) {
        case TOKEN_LOCAL_SYMBOL:
            change = true;

            if (!mac) {
                err_not_mac = true;
                break;
            }

            type = TOKEN_ID;
            text = nasm_asprintf("..@%"PRIu64".%s", mac->unique, text+2);
            break;
        case TOKEN_MMACRO_PARAM:
        {
            Token *tt = NULL;

            change = true;

            if (!mac) {
                err_not_mac = true;
                break;
            }

            if (strchr(text, ':')) {
                /* It is a range */
                expand_mmac_params_range(mac, t, &tail);
                text = NULL;
                break;
            }

            switch (text[1]) {
                /*
                 * We have to make a substitution of one of the
                 * forms %1, %-1, %+1, %%foo, %0, %00.
                 */
            case '0':
                if (!text[2]) {
                    type = TOKEN_NUM;
                    text = nasm_asprintf("%d", mac->nparam);
                    break;
                }
                if (text[2] != '0' || text[3])
                    goto invalid;
                /* a possible captured label == mac->params[0] */
                /* fall through */
            default:
            {
                unsigned long n;
                char *ep;

                n = strtoul(text + 1, &ep, 10);
                if (unlikely(*ep))
                    goto invalid;

                if (n <= mac->nparam) {
                    n = mmac_rotate(mac, n);
                    dup_tlistn(mac->params[n], mac->paramlen[n], &tail);
                }
                text = NULL;
                break;
            }
            case '-':
            case '+':
            {
                int cc;
                unsigned long n;
                char *ep;

                n = strtoul(tok_text(t) + 2, &ep, 10);
                if (unlikely(*ep))
                    goto invalid;

                if (n && n <= mac->nparam) {
                    n = mmac_rotate(mac, n);
                    tt = mac->params[n];
                }
                cc = find_cc(tt);
                if (cc == -1) {
                    nasm_nonfatal("macro parameter `%s' is not a condition code",
                                  tok_text(t));
                    text = NULL;
                    break;
                }

                type = TOKEN_ID;
                if (text[1] == '-') {
                    int ncc = inverse_ccs[cc];
                    if (unlikely(ncc == -1)) {
                        nasm_nonfatal("condition code `%s' is not invertible",
                                      conditions[cc]);
                        break;
                    }
                    cc = ncc;
                }
                text = nasm_strdup(conditions[cc]);
                break;
            }

            invalid:
                nasm_nonfatal("invalid macro parameter: `%s'", text);
                text = NULL;
                break;
            }
            break;
        }

        case TOKEN_PREPROC_Q:
            if (mac) {
                type = TOKEN_ID;
                text = nasm_strdup(mac->iname);
                change = true;
            } else {
                change = false;
            }
            break;

        case TOKEN_PREPROC_QQ:
            if (mac) {
                type = TOKEN_ID;
                text = nasm_strdup(mac->name);
                change = true;
            } else {
                change = false;
            }
            break;

        case TOKEN_INDIRECT:
        {
            Token *tt;

            tt = tokenize(tok_text(t));
            tt = expand_mmac_params(tt);
            tt = expand_smacro(tt);
            tail = steal_tlist(tt, tail);
            text = NULL;
            change = true;
            break;
        }

        default:
            change = false;
            break;
        }

        if (err_not_mac) {
            nasm_nonfatal("`%s': not in a macro call", text);
            text = NULL;
            change = true;
        }

        if (change) {
            if (!text) {
                delete_Token(t);
            } else {
                *tail = t;
                tail = &t->next;
		set_text(t, text, tok_strlen(text));
                t->type = type;
            }
            changed = true;
        } else {
            *tail = t;
            tail = &t->next;
        }
    }

    *tail = NULL;

    if (changed) {
        const struct concat_mask t[] = {
            {
                CONCAT_ID | CONCAT_FLOAT,   /* head */
                CONCAT_ID | CONCAT_NUM | CONCAT_FLOAT | CONCAT_OP /* tail */
            },
            {
                CONCAT_NUM,     /* head */
                CONCAT_NUM      /* tail */
            }
        };
        paste_tokens(&thead, t, ARRAY_SIZE(t), false);
    }

    return thead;
}

static Token *expand_smacro_noreset(Token * tline);
static SMacro *expand_one_smacro(Token ***tpp);

/*
 * Expand one single-line macro instance given a specific macro and a
 * specific set of parameters. Returns a pointer to the expansion, and
 * the pointer *epp pointing to the next pointer of the last token of
 * the expansion; if the expansion is empty return NULL and *epp is
 * unchanged.
 *
 * mstart is the token containing the token name *as invoked*.
 */
static Token *
expand_smacro_with_params(SMacro *m, Token *mstart, Token **params,
                          int nparam, Token ***epp)
{
    /* Is it a macro or a preprocessor function? Used for diagnostics. */
    const char * const mtype = m->name[0] == '%' ? "function" : "macro";
    Token *t, *tline, *tup;
    bool cond_comma;
    const struct smac_param *mparm;
    int i;

    /* Expand the macro */
    m->in_progress++;

    /*
     * Postprocessing of of parameters. Note that the ordering matters
     * here.
     *
     * mparm points to the current parameter specification
     * structure (struct smac_param); this may not match the index
     * i in the case of varadic parameters.
     */
    if (nparam) {
        for (i = 0, mparm = m->params; i < nparam;
             i++, mparm += !(mparm->flags & SPARM_VARADIC)) {
            const enum sparmflags flags = mparm->flags;

            if (flags & SPARM_EVAL) {
                /* Evaluate this parameter as a number */
                struct ppscan pps;
                struct tokenval tokval;
                expr *evalresult;
                Token *eval_param;

                eval_param = zap_white(expand_smacro_noreset(params[i]));
                params[i] = NULL;

                if (!eval_param) {
                    /* empty argument */
                    if (mparm->def) {
                        params[i] = dup_tlist(mparm->def, NULL);
                        continue;
                    } else if (flags & SPARM_OPTIONAL) {
                        continue;
                    }
                    /* otherwise, allow evaluate() to generate an error */
                }

                pps.tptr = eval_param;
                pps.ntokens = -1;
                tokval.t_type = TOKEN_INVALID;
                evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);

                free_tlist(eval_param);

                if (!evalresult) {
                    /* Nothing meaningful to do */
                } else if (tokval.t_type) {
                    nasm_nonfatal("invalid expression in parameter %d of %s `%s'",
                                  i+1, mtype, m->name);
                } else if (!is_simple(evalresult)) {
                    nasm_nonfatal("non-constant expression in parameter %d of %s `%s'",
                                  i+1, mtype, m->name);
                } else {
                    int64_t v = reloc_value(evalresult);
                    params[i] = make_tok_num_radix(NULL, v, mparm->radix,
                                                   !!(flags & SPARM_UNSIGNED));
                }
            }

            if (flags & SPARM_STR) {
                /* Convert expansion to a quoted string */
                Token *qs;

                qs = expand_smacro_noreset(params[i]);
                if ((flags & SPARM_CONDQUOTE) &&
                    tok_is(qs, TOKEN_STR) && !qs->next) {
                    /* A single quoted string token */
                    params[i] = qs;
                } else {
                    char *arg = detoken(qs, false);
                    free_tlist(qs);
                    params[i] = make_tok_qstr(NULL, arg);
                    nasm_free(arg);
                }
            }
        }
    }


    /* Note: we own the expansion this returns. */
    t = m->expand(m, params, nparam);

    tup = tline = NULL;
    cond_comma = false;

    while (t) {
        enum token_type type = t->type;
        Token *tnext = t->next;

        switch (type) {
        case TOKEN_PREPROC_Q:
        case TOKEN_PREPROC_SQ:
            delete_Token(t);
            t = dup_Token(tline, mstart);
            break;

        case TOKEN_PREPROC_QQ:
        case TOKEN_PREPROC_SQQ:
        {
            size_t mlen = strlen(m->name);
	    size_t len;
            char *p, *from;

            t->type = mstart->type;
            if (t->type == TOKEN_LOCAL_MACRO) {
		const char *psp; /* prefix start pointer */
                const char *pep; /* prefix end pointer */
		size_t plen;

		psp = tok_text(mstart);
                get_ctx(psp, &pep);
                plen = pep - psp;

                len = mlen + plen;
                from = p = nasm_malloc(len + 1);
                p = mempcpy(p, psp, plen);
            } else {
                len = mlen;
                from = p = nasm_malloc(len + 1);
            }
            p = mempcpy(p, m->name, mlen);
            *p = '\0';
	    set_text_free(t, from, len);

            t->next = tline;
            break;
        }

        case TOKEN_COND_COMMA:
            delete_Token(t);
            t = cond_comma ? make_tok_char(tline, ',') : NULL;
            break;

        case TOKEN_ID:
        case TOKEN_PREPROC_ID:
	case TOKEN_LOCAL_MACRO:
        {
            /*
             * Chain this into the target line *before* expanding,
             * that way we pick up any arguments to the new macro call,
             * if applicable.
             */
            Token **tp = &t;
            t->next = tline;
            expand_one_smacro(&tp);
            tline = *tp;        /* First token left after any macro call */
            break;
        }
        default:
            if (is_smac_param(t->type)) {
                int param = smac_nparam(t->type);
                nasm_assert(!tup && param < nparam);
                delete_Token(t);
                t = NULL;
                tup = tnext;
                tnext = dup_tlist_reverse(params[param], NULL);
                cond_comma = false;
            } else {
                t->next = tline;
            }
        }

        if (t) {
            Token *endt = tline;

            tline = t;
            while (!cond_comma && t && t != endt) {
                cond_comma = t->type != TOKEN_WHITESPACE;
                t = t->next;
            }
        }

        if (tnext) {
            t = tnext;
        } else {
            t = tup;
            tup = NULL;
        }
    }

    if (epp) {
        Token **ep = *epp;
        for (t = tline; t; t = t->next)
            ep = &t->next;
        *epp = ep;
    }

    /* Expansion complete */
    m->in_progress--;

    return tline;
}

/*
 * Count the arguments to an smacro call. Returns 0 if the token following
 * is not a left paren. *tp is set to point to the final ) if non-NULL;
 * it is left unchanged for the zero-argument case.
 */
static int count_smacro_args(Token *t, Token **tp)
{
    int nparam;
    int paren, brackets;

    t = skip_white(t);

    if (!tok_is(t, '('))
        return 0;

    paren = 1;
    nparam = 1;
    brackets = 0;

    while (paren) {
        t = t->next;

        if (!t) {
            nasm_nonfatal("macro call expects terminating `)'");
            return 0;
        }

        switch (t->type) {
        case ',':
            if (!brackets && paren == 1)
                nparam++;
            break;

        case '{':
            brackets++;
            break;

        case '}':
            if (brackets > 0)
                brackets--;
            break;

        case '(':
            if (!brackets)
                paren++;
            break;

        case ')':
            if (!brackets)
                paren--;
            break;

        default:
            break;          /* Normal token */
        }
    }

    if (tp)
        *tp = t;
    return nparam;
}

/*
 * Collect the arguments to an smacro call. The size of the array must have
 * been previously counted. It *is* permitted to call this with an nparam
 * value that is too small for the macro in question; in that case the
 * parameters are treated as missing optional arguments, even if they
 * are not optional in the macro specification.
 *
 * *nparamp is adjusted if some arguments got merged as greedy or entered
 * as optional/empty.
 *
 * Moves *tp to point to the final ) token.
 */
static Token **parse_smacro_args(Token **tp, int *nparamp, const SMacro *m)
{
    Token **phead, **pep;
    int white = 0;
    int brackets = 0;
    int paren;
    bool bracketed = false;
    bool bad_bracket = false;
    int i;
    enum sparmflags flags;
    const struct smac_param *mparm;
    Token *t = *tp;
    int nparam = *nparamp;
    Token **params;
    /* Is it a macro or a preprocessor function? Used for diagnostics. */
    const char * const mtype = m->name[0] == '%' ? "function" : "macro";

    t = skip_white(t);
    nasm_assert(tok_is(t, '('));

    if (nparam > m->nparam) {
        if (m->params[m->nparam-1].flags & SPARM_GREEDY)
            *nparamp = nparam = m->nparam;
    } else if (nparam < m->nparam) {
        *nparamp = nparam = m->nparam; /* Missing optional arguments = empty */
    }
    paren = 1;
    nasm_newn(params, nparam);
    i = 0;
    mparm = m->params;
    flags = mparm->flags;
    phead = pep = &params[i];
    *pep = NULL;

    while (paren) {
        bool skip;

        t = t->next;

        if (!t)
            nasm_nonfatal("%s `%s' call expects terminating `)'",
                          mtype, m->name);

        skip = false;

        switch (t->type) {
        case TOKEN_WHITESPACE:
            if (!(flags & SPARM_NOSTRIP)) {
                if (brackets || *phead)
                    white++;    /* Keep interior whitespace */
                skip = true;
            }
            break;

        case ',':
            if (!brackets && paren == 1 && !(flags & SPARM_GREEDY)) {
                i++;
                nasm_assert(i < nparam);
                phead = pep = &params[i];
                *pep = NULL;
                bracketed = false;
                skip = true;
                if (!(flags & SPARM_VARADIC)) {
                    mparm++;
                    flags = mparm->flags;
                }
            }
            break;

        case '{':
            if (!bracketed) {
                bracketed = !*phead && !(flags & SPARM_NOSTRIP);
                skip = bracketed;
            }
            brackets++;
            break;

        case '}':
            if (brackets > 0) {
                if (!--brackets)
                    skip = bracketed;
            }
            break;

        case '(':
            if (!brackets)
                paren++;
            break;

        case ')':
            if (!brackets) {
                paren--;
                if (!paren) {
                    skip = true;
                    i++;    /* Found last argument */
                }
            }
            break;

        default:
            break;          /* Normal token */
        }

        if (!skip) {
            Token *tt;

            bad_bracket |= bracketed && !brackets;

            if (white) {
                *pep = tt = new_White(NULL);
                pep = &tt->next;
                white = 0;
            }
            *pep = tt = dup_Token(NULL, t);
            pep = &tt->next;
        }
    }

    *tp = t;
    return params;
}

/*
 * Expand *one* single-line macro instance. If the first token is not
 * a macro at all, it is simply copied to the output and the pointer
 * advanced.  tpp should be a pointer to a pointer (usually the next
 * pointer of the previous token) to the first token. **tpp is updated
 * to point to the first token of the expansion, and *tpp updated to
 * point to the next pointer of the last token of the expansion.
 *
 * If the expansion is empty, *tpp will be unchanged but **tpp will
 * be advanced past the macro call.
 *
 * Return the macro expanded, or NULL if no expansion took place.
 */
static SMacro *expand_one_smacro(Token ***tpp)
{
    Token **params = NULL;
    const char *mname;
    Token *mstart = **tpp;
    Token *tline  = mstart;
    SMacro *head, *m;
    Token *tafter, **tep;
    int nparam = 0;

    if (!tline)
        return false;           /* Empty line, nothing to do */

    mname = tok_text(mstart);

    smacro_deadman.total--;
    smacro_deadman.levels--;

    if (unlikely(smacro_deadman.total < 0 || smacro_deadman.levels < 0)) {
        if (unlikely(!smacro_deadman.triggered)) {
            nasm_nonfatal("interminable macro recursion");
            smacro_deadman.triggered = true;
        }
        goto not_a_macro;
    } else if (tline->type == TOKEN_ID || tline->type == TOKEN_PREPROC_ID) {
        head = (SMacro *)hash_findix(&smacros, mname);
    } else if (tline->type == TOKEN_LOCAL_MACRO) {
        Context *ctx = get_ctx(mname, &mname);
        head = ctx ? (SMacro *)hash_findix(&ctx->localmac, mname) : NULL;
    } else {
        goto not_a_macro;
    }

    /*
     * We've hit an identifier of some sort. First check whether the
     * identifier is a single-line macro at all, then think about
     * checking for parameters if necessary.
     */
    list_for_each(m, head) {
        if (unlikely(m->alias && ppconf.noaliases))
            continue;
        if (!mstrcmp(m->name, mname, m->casesense))
            break;
    }

    if (!m) {
        goto not_a_macro;
    }

    /* Parse parameters, if applicable */

    params = NULL;
    nparam = 0;

    if (m->nparam == 0) {
        /*
         * Simple case: the macro is parameterless.
         * Nothing to parse; the expansion code will
         * drop the macro name token.
         */
    } else {
        /*
         * Complicated case: at least one macro with this name
         * exists and takes parameters. We must find the
         * parameters in the call, count them, find the SMacro
         * that corresponds to that form of the macro call, and
         * substitute for the parameters when we expand. What a
         * pain.
         */
        tline = skip_white(tline->next);
        nparam = count_smacro_args(tline, NULL);
        if (!nparam)
            goto not_a_macro;

        /*
         * Look for a macro matching in both name and parameter count.
         * We already know any matches cannot be anywhere before the
         * current position of "m", so there is no reason to
         * backtrack.
         */
        while (1) {
            if (!m) {
                /*!
                 *!pp-macro-params-single [on] single-line macro calls with wrong parameter count
                 *!=macro-params-single
                 *!  warns about \i{single-line macros} being invoked
                 *!  with the wrong number of parameters.
                 */
                nasm_warn(WARN_PP_MACRO_PARAMS_SINGLE|ERR_HOLD,
                    "single-line macro `%s' exists, "
                    "but not taking %d parameter%s",
                    mname, nparam, (nparam == 1) ? "" : "s");
                goto not_a_macro;
            }

            if (!mstrcmp(m->name, mname, m->casesense)) {
                if (nparam >= m->nparam_min &&
                    (m->varadic || nparam <= m->nparam))
                    break;      /* It's good */
            }
            m = m->next;
        }
    }

    if (m->in_progress && !m->recursive)
        goto not_a_macro;

   if (nparam) {
        params = parse_smacro_args(&tline, &nparam, m);
    }

    tafter = tline->next;   /* Skip past the macro call */
    tline->next = NULL;     /* Truncate mstart list at the macro call end */
    tline = expand_smacro_with_params(m, mstart, params, nparam, &tep);
    if (tline) {
        **tpp = tline;
        *tep = tafter;
        *tpp = tep;
    } else {
        **tpp = tafter;
    }

    /* Don't do this until after expansion or we will clobber mname */
    free_tlist(mstart);
    goto done;

    /*
     * No macro expansion needed; roll back to mstart (if necessary)
     * and then advance to the next input token. Note that this is
     * by far the common case!
     */
not_a_macro:
    *tpp = &mstart->next;
    m = NULL;
done:
    free_tlist_array(params, nparam);
    smacro_deadman.levels++;
    return m;
}

/*
 * Expand all single-line macro calls made in the given line.
 * Return the expanded version of the line. The original is deemed
 * to be destroyed in the process. (In reality we'll just move
 * Tokens from input to output a lot of the time, rather than
 * actually bothering to destroy and replicate.)
 */
static Token *expand_smacro(Token *tline)
{
    smacro_deadman.total  = nasm_limit[LIMIT_MACRO_TOKENS];
    smacro_deadman.levels = nasm_limit[LIMIT_MACRO_LEVELS];
    smacro_deadman.triggered = false;
    return expand_smacro_noreset(tline);
}

static Token *expand_smacro_noreset(Token *org_tline)
{
    Token *tline;
    bool expanded;
    errhold errhold;       /* Hold warning/errors during expansion */

    if (!org_tline)
        return NULL;            /* Empty input */

    /*
     * Trick: we should avoid changing the start token pointer since it can
     * be contained in "next" field of other token. Because of this
     * we allocate a copy of first token and work with it; at the end of
     * routine we copy it back
     */
    tline = dup_Token(org_tline->next, org_tline);

    /*
     * Pretend that we always end up doing expansion on the first pass;
     * that way %+ get processed. However, if we process %+ before the
     * first pass, we end up with things like MACRO %+ TAIL trying to
     * look up the macro "MACROTAIL", which we don't want.
     */
    expanded = true;

    while (true) {
        static const struct concat_mask tmatch[] = {
            {
                CONCAT_ID | CONCAT_LOCAL_MACRO |
                CONCAT_ENVIRON | CONCAT_PREPROC_ID, /* head */
                CONCAT_ID | CONCAT_LOCAL_MACRO |
                CONCAT_ENVIRON | CONCAT_PREPROC_ID |
                CONCAT_NUM      /* tail */
            }
        };
        Token **tail = &tline;

        /*
         * We hold warnings/errors until we are done in this loop. It is
         * possible for nuisance warnings to appear that disappear on later
         * passes.
         */
        errhold = nasm_error_hold_push();

        while (*tail)           /* main token loop */
            expanded |= !!expand_one_smacro(&tail);

         if (!expanded)
            break;              /* Done! */

        /*
         * Now scan the entire line and look for successive TOKEN_IDs
         * that resulted after expansion (they can't be produced by
         * tokenize()). The successive TOKEN_IDs should be concatenated.
         * Also we look for %+ tokens and concatenate the tokens
         * before and after them (without white spaces in between).
         */
        if (!paste_tokens(&tline, tmatch, ARRAY_SIZE(tmatch), true))
            break;              /* Done again! */

        nasm_error_hold_pop(errhold, false);
        expanded = false;
    }
    nasm_error_hold_pop(errhold, true);

    if (!tline) {
        /*
         * The expression expanded to empty line;
         * we can't return NULL because of the "trick" above.
         * Just set the line to a single WHITESPACE token.
	 */

	tline = new_White(NULL);
    }

    steal_Token(org_tline, tline);
    org_tline->next = tline->next;
    delete_Token(tline);

    return org_tline;
}

/*
 * Similar to expand_smacro but used exclusively with macro identifiers
 * right before they are fetched in. The reason is that there can be
 * identifiers consisting of several subparts. We consider that if there
 * are more than one element forming the name, user wants a expansion,
 * otherwise it will be left as-is. Example:
 *
 *      %define %$abc cde
 *
 * the identifier %$abc will be left as-is so that the handler for %define
 * will suck it and define the corresponding value. Other case:
 *
 *      %define _%$abc cde
 *
 * In this case user wants name to be expanded *before* %define starts
 * working, so we'll expand %$abc into something (if it has a value;
 * otherwise it will be left as-is) then concatenate all successive
 * PP_IDs into one.
 */
static Token *expand_id(Token * tline)
{
    Token *cur, *oldnext = NULL;

    if (!tline || !tline->next)
        return tline;

    cur = tline;
    while (cur->next &&
           (cur->next->type == TOKEN_ID ||
            cur->next->type == TOKEN_PREPROC_ID ||
            cur->next->type == TOKEN_LOCAL_MACRO ||
            cur->next->type == TOKEN_NUM))
        cur = cur->next;

    /* If identifier consists of just one token, don't expand */
    if (cur == tline)
        return tline;

    if (cur) {
        oldnext = cur->next;    /* Detach the tail past identifier */
        cur->next = NULL;       /* so that expand_smacro stops here */
    }

    tline = expand_smacro(tline);

    if (cur) {
        /* expand_smacro possibly changhed tline; re-scan for EOL */
        cur = tline;
        while (cur && cur->next)
            cur = cur->next;
        if (cur)
            cur->next = oldnext;
    }

    return tline;
}

/*
 * This is called from find_mmacro_in_list() after finding a suitable macro.
 */
static MMacro *use_mmacro(MMacro *m, int *nparamp, Token ***paramsp)
{
    int nparam = *nparamp;
    Token **params = *paramsp;

    /*
     * This one is right. Just check if cycle removal
     * prohibits us using it before we actually celebrate...
     */
    if (m->in_progress > m->max_depth) {
        if (m->max_depth > 0) {
            /* Document this properly when recursive mmacros re-implemented */
            nasm_warn(WARN_OTHER, "reached maximum recursion depth of %i",
                      m->max_depth);
        }
        nasm_free(params);
        *nparamp = 0;
        *paramsp = NULL;
        return NULL;
    }

    /*
     * It's right, and we can use it. Add its default
     * parameters to the end of our list if necessary.
     */
    if (m->defaults && nparam < m->nparam_min + m->ndefs) {
        int newnparam = m->nparam_min + m->ndefs;
        params = nasm_realloc(params, sizeof(*params) * (newnparam+2));
        memcpy(&params[nparam+1], &m->defaults[nparam+1-m->nparam_min],
               (newnparam - nparam) * sizeof(*params));
        nparam = newnparam;
    }
    /*
     * If we've gone over the maximum parameter count (and
     * we're in Plus mode), ignore parameters beyond
     * nparam_max.
     */
    if (m->plus && nparam > m->nparam_max)
        nparam = m->nparam_max;

    /*
     * If nparam was adjusted above, make sure the list is still
     * NULL-terminated.
     */
    params[nparam+1] = NULL;

    /* Done! */
    *paramsp = params;
    *nparamp = nparam;
    return m;
}

/*
 * Search a macro list and try to find a match. If matching, call
 * use_mmacro() to set up the macro call. m points to the list of
 * search, which is_mmacro() sets to the first *possible* match.
 */
static MMacro *
find_mmacro_in_list(MMacro *m, const char *finding,
                    int *nparamp, Token ***paramsp)
{
    int nparam = *nparamp;

    while (m) {
        if (m->nparam_min <= nparam
            && (m->plus || nparam <= m->nparam_max)) {
            /*
             * This one matches, use it.
             */
            return use_mmacro(m, nparamp, paramsp);
        }

        /*
         * Otherwise search for the next one with a name match.
         */
        list_for_each(m, m->next) {
            if (!mstrcmp(m->name, finding, m->casesense))
                break;
        }
    }

    return NULL;
}

/*
 * Determine whether the given line constitutes a multi-line macro
 * call, and return the MMacro structure called if so. Doesn't have
 * to check for an initial label - that's taken care of in
 * expand_mmacro - but must check numbers of parameters. Guaranteed
 * to be called with tline->type == TOKEN_ID, so the putative macro
 * name is easy to find.
 */
static MMacro *is_mmacro(Token * tline, int *nparamp, Token ***paramsp)
{
    MMacro *head, *m, *found;
    Token **params, **comma;
    int raw_nparam, nparam;
    const char *finding = tok_text(tline);
    bool empty_args = !tline->next;

    *nparamp = 0;
    *paramsp = NULL;

    head = (MMacro *) hash_findix(&mmacros, finding);

    /*
     * Efficiency: first we see if any macro exists with the given
     * name which isn't already excluded by macro cycle removal.
     * (The cycle removal test here helps optimize the case of wrapping
     * instructions, and is cheap to do here.)
     *
     * If not, we can return NULL immediately. _Then_ we
     * count the parameters, and then we look further along the
     * list if necessary to find the proper MMacro.
     */
    list_for_each(m, head) {
        if (!mstrcmp(m->name, finding, m->casesense) &&
            (m->in_progress != 1 || m->max_depth > 0))
            break;              /* Found something that needs consideration */
    }
    if (!m)
        return NULL;

    /*
     * OK, we have a potential macro. Count and demarcate the
     * parameters.
     */
    comma = count_mmac_params(tline->next, nparamp, paramsp);
    raw_nparam = *nparamp;

    /*
     * Search for an exact match. This cannot come *before* the m
     * found in the list search before, so we can start there.
     *
     * If found is NULL and *paramsp has been cleared, then we
     * encountered an error for which we have already issued a
     * diagnostic, so we should not proceed.
     */
    found = find_mmacro_in_list(m, finding, nparamp, paramsp);
    if (!*paramsp)
        return NULL;

    nparam = *nparamp;
    params = *paramsp;

    /*
     * Special weirdness: in NASM < 2.15, an expansion of
     * *only* whitespace, as can happen during macro expansion under
     * certain circumstances, is counted as zero arguments for the
     * purpose of %0, but one argument for the purpose of macro
     * matching! In particular, this affects:
     *
     * foobar %1
     *
     * ... with %1 being empty; this would call the one-argument
     * version of "foobar" with an empty argument, equivalent to
     *
     * foobar {%1}
     *
     * ... except that %0 would be set to 0 inside foobar, even if
     * foobar is declared with "%macro foobar 1" or equivalent!
     *
     * The proper way to do that is to define "%macro foobar 0-1".
     *
     * To be compatible without doing something too stupid, try to
     * match a zero-argument macro first, but if that fails, try
     * for a one-argument macro with the above behavior.
     *
     * Furthermore, NASM < 2.15 will match stripping a tailing empty
     * argument, but in that case %0 *does* reflect that this argument
     * have been stripped; this is handled in count_mmac_params().
     *
     * To disable these insane legacy behaviors, use:
     *
     * %pragma preproc sane_empty_expansion yes
     *
     *!pp-macro-params-legacy [on] improperly calling multi-line macro for legacy support
     *!=macro-params-legacy
     *!  warns about \i{multi-line macros} being invoked
     *!  with the wrong number of parameters, but for bug-compatibility
     *!  with NASM versions older than 2.15, NASM tried to fix up the
     *!  parameters to match the legacy behavior and call the macro anyway.
     *!  This can happen in certain cases where there are empty arguments
     *!  without braces, sometimes as a result of macro expansion.
     *!-
     *!  The legacy behavior is quite strange and highly context-dependent,
     *!  and can be disabled with:
     *!-
     *!  \c      %pragma preproc sane_empty_expansion true
     *!-
     *!  It is highly recommended to use this option in new code.
     */
    if (!ppconf.sane_empty_expansion) {
        if (!found) {
            if (raw_nparam == 0 && !empty_args) {
                /*
                 * A single all-whitespace parameter as the only thing?
                 * Look for a one-argument macro, but don't adjust
                 * *nparamp.
                 */
                int bogus_nparam = 1;
                params[2] = NULL;
                found = find_mmacro_in_list(m, finding, &bogus_nparam, paramsp);
            } else if (raw_nparam > 1 && comma) {
                Token *comma_tail = *comma;

                /*
                 * Drop the terminal argument and try again.
                 * If we fail, we need to restore the comma to
                 * preserve tlist.
                 */
                *comma = NULL;
                *nparamp = raw_nparam - 1;
                found = find_mmacro_in_list(m, finding, nparamp, paramsp);
                if (found)
                    free_tlist(comma_tail);
                else
                    *comma = comma_tail;
            }

            if (!*paramsp)
                return NULL;
        } else if (comma) {
            free_tlist(*comma);
            *comma = NULL;
            if (raw_nparam > found->nparam_min &&
                raw_nparam <= found->nparam_min + found->ndefs) {
                /* Replace empty argument with default parameter */
                params[raw_nparam] =
                    found->defaults[raw_nparam - found->nparam_min];
            } else if (raw_nparam > found->nparam_max && found->plus) {
                /* Just drop the comma, don't adjust argument count */
            } else {
                /* Drop argument. This may cause nparam < nparam_min. */
                params[raw_nparam] = NULL;
                *nparamp = nparam = raw_nparam - 1;
            }
        }

        if (found) {
            if (raw_nparam < found->nparam_min ||
                (raw_nparam > found->nparam_max && !found->plus)) {
                nasm_warn(WARN_PP_MACRO_PARAMS_LEGACY,
                          "improperly calling multi-line macro `%s' with %d parameters",
                          found->name, raw_nparam);
            } else if (comma) {
                nasm_warn(WARN_PP_MACRO_PARAMS_LEGACY,
                          "dropping trailing empty parameter in call to multi-line macro `%s'", found->name);
            }
        }
    }

    /*
     * After all that, we didn't find one with the right number of
     * parameters. Issue a warning, and fail to expand the macro.
     *!
     *!pp-macro-params-multi [on] multi-line macro calls with wrong parameter count
     *!=macro-params-multi
     *!  warns about \i{multi-line macros} being invoked
     *!  with the wrong number of parameters. See \k{mlmacover} for an
     *!  example of why you might want to disable this warning.
     */
    if (found)
        return found;

    nasm_warn(WARN_PP_MACRO_PARAMS_MULTI,
               "multi-line macro `%s' exists, but not taking %d parameter%s",
              finding, nparam, (nparam == 1) ? "" : "s");
    nasm_free(*paramsp);
    return NULL;
}


#if 0

/*
 * Save MMacro invocation specific fields in
 * preparation for a recursive macro expansion
 */
static void push_mmacro(MMacro *m)
{
    MMacroInvocation *i;

    i = nasm_malloc(sizeof(MMacroInvocation));
    i->prev = m->prev;
    i->params = m->params;
    i->iline = m->iline;
    i->nparam = m->nparam;
    i->rotate = m->rotate;
    i->paramlen = m->paramlen;
    i->unique = m->unique;
    i->condcnt = m->condcnt;
    m->prev = i;
}


/*
 * Restore MMacro invocation specific fields that were
 * saved during a previous recursive macro expansion
 */
static void pop_mmacro(MMacro *m)
{
    MMacroInvocation *i;

    if (m->prev) {
        i = m->prev;
        m->prev = i->prev;
        m->params = i->params;
        m->iline = i->iline;
        m->nparam = i->nparam;
        m->rotate = i->rotate;
        m->paramlen = i->paramlen;
        m->unique = i->unique;
        m->condcnt = i->condcnt;
        nasm_free(i);
    }
}

#endif

/*
 * List an mmacro call with arguments (-Lm option)
 */
static void list_mmacro_call(const MMacro *m)
{
    const char prefix[] = " ;;; [macro] ";
    size_t namelen, size;
    char *buf, *p;
    unsigned int i;
    const Token *t;

    namelen = strlen(m->iname);
    size = namelen + sizeof(prefix); /* Includes final null (from prefix) */

    for (i = 1; i <= m->nparam; i++) {
        int j = 0;
        size += 3;              /* Braces and space/comma */
        list_for_each(t, m->params[i]) {
            if (j++ >= m->paramlen[i])
                break;
            size += (t->type == TOKEN_WHITESPACE) ? 1 : t->len;
        }
    }

    buf = p = nasm_malloc(size);
    p = mempcpy(p, prefix, sizeof(prefix) - 1);
    p = mempcpy(p, m->iname, namelen);
    *p++ = ' ';

    for (i = 1; i <= m->nparam; i++) {
        int j = 0;
        *p++ = '{';
        list_for_each(t, m->params[i]) {
            if (j++ >= m->paramlen[i])
                break;
	    p = mempcpy(p, tok_text(t), t->len);
        }
        *p++ = '}';
        *p++ = ',';
    }

    *--p = '\0';                /* Replace last delimiter with null */
    lfmt->line(LIST_MACRO, -1, buf);
    nasm_free(buf);
}

/*
 * Collect information about macro invocations for the benefit of
 * the debugger. During execution we create a reverse list; before
 * calling the backend reverse it to definition/invocation order just
 * to be nicer. [XXX: not implemented yet]
 */
struct debug_macro_inv *debug_current_macro;

/* Get/create a addr structure for a seg:inv combo */
static struct debug_macro_addr *
debug_macro_get_addr_inv(int32_t seg, struct debug_macro_inv *inv)
{
    struct debug_macro_addr *addr;
    nasm_static_assert(offsetof(struct debug_macro_addr, tree) == 0);

    if (likely(seg == inv->lastseg))
        return inv->addr.last;

    inv->lastseg = seg;
    addr = (struct debug_macro_addr *)
        rb_search_exact(inv->addr.tree, seg);
    if (unlikely(!addr)) {
        nasm_new(addr);
        addr->tree.key = seg;
        inv->addr.tree = rb_insert(inv->addr.tree, &addr->tree);
        inv->naddr++;
        if (inv->up)
            addr->up = debug_macro_get_addr_inv(seg, inv->up);
    }

    return inv->addr.last = addr;
}

/* Get/create an addr structure for a seg in debug_current_macro */
struct debug_macro_addr *debug_macro_get_addr(int32_t seg)
{
    return debug_macro_get_addr_inv(seg, debug_current_macro);
}

static struct debug_macro_info dmi;
static struct debug_macro_inv_list *current_inv_list = &dmi.inv;

static void debug_macro_start(MMacro *m, struct src_location where)
{
    struct debug_macro_def *def = m->dbg.def;
    struct debug_macro_inv *inv;

    nasm_assert(!m->dbg.inv);

    /* First invocation? Need to create a def structure */
    if (unlikely(!def)) {
        nasm_new(def);
        def->name = nasm_strdup(m->name);
        def->where = m->where;

        def->next = dmi.def.l;
        dmi.def.l = def;
        dmi.def.n++;

        m->dbg.def = def;
    }

    nasm_new(inv);
    inv->lastseg = NO_SEG;
    inv->where = where;
    inv->up = debug_current_macro;
    inv->next = current_inv_list->l;
    inv->def = def;
    current_inv_list->l = inv;
    current_inv_list->n++;
    current_inv_list = &inv->down;

    def->ninv++;
    m->dbg.inv = inv;
    debug_current_macro = inv;
}

static void debug_macro_end(MMacro *m)
{
    struct debug_macro_inv *inv = m->dbg.inv;

    nasm_assert(inv == debug_current_macro);

    list_reverse(inv->down.l);

    m->dbg.inv = NULL;
    inv = inv->up;

    m = istk->mstk.mmac;
    if (m) {
        nasm_assert(inv == m->dbg.inv);
        debug_current_macro = inv;
        current_inv_list = &inv->down;
    } else {
        nasm_assert(!inv);
        debug_current_macro = NULL;
        current_inv_list = &dmi.inv;
    }
}

static void free_debug_macro_addr_tree(struct rbtree *tree)
{
    struct rbtree *left, *right;
    nasm_static_assert(offsetof(struct debug_macro_addr,tree) == 0);

    if (!tree)
        return;

    left  = rb_left(tree);
    right = rb_right(tree);

    nasm_free(tree);

    free_debug_macro_addr_tree(left);
    free_debug_macro_addr_tree(right);
}

static void free_debug_macro_inv_list(struct debug_macro_inv *inv)
{
    struct debug_macro_inv *itmp;

    if (!inv)
        return;

    list_for_each_safe(inv, itmp, inv) {
        free_debug_macro_inv_list(inv->down.l);
        free_debug_macro_addr_tree(inv->addr.tree);
        nasm_free(inv);
    }
}

static void free_debug_macro_info(void)
{
    struct debug_macro_def *def, *dtmp;

    list_for_each_safe(def, dtmp, dmi.def.l)
        nasm_free(def);

    free_debug_macro_inv_list(dmi.inv.l);

    nasm_zero(dmi);
}

static void debug_macro_output(void)
{
    list_reverse(dmi.inv.l);
    dfmt->debug_mmacros(&dmi);
    free_debug_macro_info();
}

/*
 * Expand the multi-line macro call made by the given line, if
 * there is one to be expanded. If there is, push the expansion on
 * istk->expansion and return 1. Otherwise return 0.
 */
static int expand_mmacro(Token * tline)
{
    Token *startline = tline;
    Token *label = NULL;
    bool dont_prepend = false;
    Token **params, *t, *tt;
    MMacro *m;
    Line *l, *ll;
    int i, *paramlen;
    const char *mname;
    int nparam = 0;

    t = tline;
    t = skip_white(t);
    if (!tok_is(t, TOKEN_ID) && !tok_is(t, TOKEN_LOCAL_MACRO))
        return 0;
    m = is_mmacro(t, &nparam, &params);
    if (m) {
        mname = tok_text(t);
    } else {
        Token *last;
        /*
         * We have an id which isn't a macro call. We'll assume
         * it might be a label; we'll also check to see if a
         * colon follows it. Then, if there's another id after
         * that lot, we'll check it again for macro-hood.
         */
        label = last = t;
        t = t->next;
        if (tok_white(t))
            last = t, t = t->next;
        if (tok_is(t, ':')) {
            dont_prepend = true;
            last = t, t = t->next;
            if (tok_white(t))
                last = t, t = t->next;
        }
        if (!tok_is(t, TOKEN_ID) || !(m = is_mmacro(t, &nparam, &params)))
            return 0;
        last->next = NULL;
        mname = tok_text(t);
        tline = t;
    }

    if (unlikely(mmacro_deadman.total >= nasm_limit[LIMIT_MMACROS] ||
                 mmacro_deadman.levels >= nasm_limit[LIMIT_MACRO_LEVELS])) {
        if (!mmacro_deadman.triggered) {
            nasm_nonfatal("interminable multiline macro recursion");
            mmacro_deadman.triggered = true;
        }
        return 0;
    }

    mmacro_deadman.total++;
    mmacro_deadman.levels++;

    /*
     * Fix up the parameters: this involves stripping leading and
     * trailing whitespace and stripping braces if they are present.
     */
    nasm_newn(paramlen, nparam+1);

    for (i = 1; (t = params[i]); i++) {
        bool braced = false;
        int brace = 0;
        int white = 0;
        bool comma = !m->plus || i < nparam;

        t = skip_white(t);
        if (tok_is(t, '{')) {
            t = t->next;
            brace = 1;
            braced = true;
            comma = false;
        }

        params[i] = t;
        for (; t; t = t->next) {
            if (tok_white(t)) {
                white++;
                continue;
            }

            switch(t->type) {
            case ',':
                if (comma && !brace)
                    goto endparam;
                break;

            case '{':
                brace++;
                break;

            case '}':
                brace--;
                if (braced && !brace) {
                    paramlen[i] += white;
                    goto endparam;
                }
                break;

            default:
                break;
            }

            paramlen[i] += white + 1;
            white = 0;
        }
    endparam:
        ;
    }

    /*
     * OK, we have a MMacro structure together with a set of
     * parameters. We must now go through the expansion and push
     * copies of each Line on to istk->expansion. Substitution of
     * parameter tokens and macro-local tokens doesn't get done
     * until the single-line macro substitution process; this is
     * because delaying them allows us to change the semantics
     * later through %rotate and give the right semantics for
     * nested mmacros.
     *
     * First, push an end marker on to istk->expansion, mark this
     * macro as in progress, and set up its invocation-specific
     * variables.
     */
    nasm_new(ll);
    ll->next = istk->expansion;
    ll->finishes = m;
    ll->where = istk->where;
    istk->expansion = ll;

    /*
     * Save the previous MMacro expansion in the case of
     * macro recursion
     */
#if 0
    if (m->max_depth && m->in_progress)
        push_mmacro(m);
#endif

    m->in_progress ++;
    m->params = params;
    m->iline = tline;
    m->iname = nasm_strdup(mname);
    m->nparam = nparam;
    m->rotate = 0;
    m->paramlen = paramlen;
    m->unique = unique++;
    m->condcnt = 0;

    m->mstk = istk->mstk;
    istk->mstk.mstk = istk->mstk.mmac = m;

    list_for_each(l, m->expansion) {
        nasm_new(ll);
        ll->next = istk->expansion;
        istk->expansion = ll;
        ll->first = dup_tlist(l->first, NULL);
        ll->where = l->where;
    }

    /*
     * If we had a label, and this macro definition does not include
     * a %00, push it on as the first line of, ot
     * the macro expansion.
     */
    if (label) {
        /*
         * We had a label. If this macro contains an %00 parameter,
         * save the value as a special parameter (which is what it
         * is), otherwise push it as the first line of the macro
         * expansion.
         */
        if (m->capture_label) {
            params[0] = dup_Token(NULL, label);
            paramlen[0] = 1;
            free_tlist(startline);
       } else {
            nasm_new(ll);
            ll->finishes = NULL;
            ll->next = istk->expansion;
            istk->expansion = ll;
            ll->first = startline;
            ll->where = istk->where;
            if (!dont_prepend) {
                while (label->next)
                    label = label->next;
                label->next = tt = make_tok_char(NULL, ':');
            }
        }
    }

    istk->nolist += !!(m->nolist & NL_LIST);
    istk->noline += !!(m->nolist & NL_LINE);

    if (!istk->nolist) {
        if (list_option('m'))
            list_mmacro_call(m);

        lfmt->uplevel(LIST_MACRO, 0);

        if (ppdbg & PDBG_MMACROS)
            debug_macro_start(m, src_where());
    }

    if (!istk->noline)
        src_macro_push(m, istk->where);

    return 1;
}

/*
 * This function decides if an error message should be suppressed.
 * It will never be called with a severity level of ERR_FATAL or
 * higher.
 */
bool pp_suppress_error(errflags severity)
{
    /*
     * If we're in a dead branch of IF or something like it, ignore the error.
     * However, because %else etc are evaluated in the state context
     * of the previous branch, errors might get lost:
     *   %if 0 ... %else trailing garbage ... %endif
     * So %else etc should set the ERR_PP_PRECOND flag.
     */
    if (istk && istk->conds &&
	((severity & ERR_PP_PRECOND) ?
	 istk->conds->state == COND_NEVER :
	 !emitting(istk->conds->state)))
        return true;

    return false;
}

static Token *
stdmac_file(const SMacro *s, Token **params, int nparams)
{
    const char *fname = src_get_fname();

    (void)s;
    (void)params;
    (void)nparams;

    return fname ? make_tok_qstr(NULL, fname) : NULL;
}

static Token *
stdmac_line(const SMacro *s, Token **params, int nparams)
{
    (void)s;
    (void)params;
    (void)nparams;

    return make_tok_num(NULL, src_get_linnum());
}

static Token *
stdmac_bits(const SMacro *s, Token **params, int nparams)
{
    (void)s;
    (void)params;
    (void)nparams;

    return make_tok_num(NULL, globalbits);
}

static Token *
stdmac_ptr(const SMacro *s, Token **params, int nparams)
{
    (void)s;
    (void)params;
    (void)nparams;

    switch (globalbits) {
    case 16:
	return new_Token(NULL, TOKEN_ID, "word", 4);
    case 32:
	return new_Token(NULL, TOKEN_ID, "dword", 5);
    case 64:
	return new_Token(NULL, TOKEN_ID, "qword", 5);
    default:
        panic();
    }
}

/* %is...() function macros */
static Token *
stdmac_is(const SMacro *s, Token **params, int nparams)
{
    int retval;
    struct Token *pline = params[0];

    (void)nparams;

    params[0] = NULL;           /* Don't free this later */

    retval = if_condition(pline, s->expandpvt.u) == COND_IF_TRUE;
    return make_tok_num(NULL, retval);
}

/*
 * Join all expanded macro arguments with commas, e.g. %eval().
 * Remember that this needs to output the tokens in reverse order.
 *
 * This can also be used when only single argument is already ready
 * to be emitted, e.g. %str().
 */
static Token *
stdmac_join(const SMacro *s, Token **params, int nparams)
{
    struct Token *tline = NULL;
    int i;

    (void)s;

    for (i = 0; i < nparams; i++) {
        Token *t, *ttmp;

        if (i)
            tline = make_tok_char(tline, ',');

        list_for_each_safe(t, ttmp, params[i]) {
            t->next = tline;
            tline = t;
        }

        /* Avoid freeing the tokens we "stole" */
        params[i] = NULL;
    }

    return tline;
}

/* %strcat() function */
static Token *
stdmac_strcat(const SMacro *s, Token **params, int nparams)
{
    int i;
    size_t len = 0;
    char *str, *p;

    (void)s;

    for (i = 0; i < nparams; i++) {
        unquote_token(params[i]);
        len += params[i]->len;
    }

    nasm_newn(str, len+1);
    p = str;

    for (i = 0; i < nparams; i++) {
        p = mempcpy(p, tok_text(params[i]), params[i]->len);
    }

    return make_tok_qstr_len(NULL, str, len);
}

/* %substr() function */
static Token *
stdmac_substr(const SMacro *s, Token **params, int nparams)
{
    int64_t start, count;

    (void)nparams;
    (void)s;

    start = get_tok_num(params[1], NULL);
    count = get_tok_num(params[2], NULL);

    return pp_substr_common(params[0], start, count);
}

/* %strlen() function */
static Token *
stdmac_strlen(const SMacro *s, Token **params, int nparams)
{
    (void)nparams;
    (void)s;

    unquote_token(params[0]);
    return make_tok_num(NULL, params[0]->len);
}

/* %tok() function */
static Token *
stdmac_tok(const SMacro *s, Token **params, int nparams)
{
    (void)nparams;
    (void)s;

    return reverse_tokens(tokenize(unquote_token_cstr(params[0])));
}

/* %cond() or %sel() */
static Token *
stdmac_cond_sel(const SMacro *s, Token **params, int nparams)
{
    int64_t which;

    /*
     * params[0] will have been generated by make_tok_num.
     */
    which = get_tok_num(params[0], NULL);

    if (s->expandpvt.u) {
        /* Booleanize (for %cond): true -> 1, false -> 2 (else) */
        which = which ? 1 : 2;
        if (which >= nparams) {
            /* false, and no else clause */
            return NULL;
        }
    } else {
            /*!
             *!pp-sel-range [on] \c{%sel()} argument out of range
             *!  warns that the \c{%sel()} preprocessor function was passed
             *!  a value less than 1 or larger than the number of available
             *!  arguments.
             */
        if (unlikely(which < 1)) {
            nasm_warn(WARN_PP_SEL_RANGE,
                      "%s(%"PRId64") is not a valid selector", s->name, which);
            return NULL;
        } else if (unlikely(which >= nparams)) {
            nasm_warn(WARN_PP_SEL_RANGE,
                      "%s(%"PRId64") exceeds the number of arguments",
                      s->name, which);
            return NULL;
        }
    }

    return new_Token(NULL, tok_smac_param(which), "", 0);
}

/* %count() function */
static Token *
stdmac_count(const SMacro *s, Token **params, int nparams)
{
    (void)s;
    (void)params;

    return make_tok_num(NULL, nparams);
}

/* %num() function */
static Token *
stdmac_num(const SMacro *s, Token **params, int nparam)
{
    int64_t parm[3];
    uint64_t n;
    int64_t dparm, bparm;
    const int maxlen = 256;
    char numbuf[256+5];
    char *p;
    char decorate;
    int i;

    (void)nparam;

    for (i = 0; i < (int)ARRAY_SIZE(parm); i++)
        parm[i] = get_tok_num(params[i], NULL);

    n      = parm[0];
    dparm  = parm[1];
    bparm  = parm[2];

    decorate = 0;
    if (bparm < 0) {
        bparm = -bparm;
        switch (bparm) {
        case 2:
            decorate = 'b';
            break;
        case 8:
            decorate = 'q';
            break;
        case 10:
            decorate = 'd';
            break;
        case 16:
            decorate = 'x';
            break;
        default:
            bparm = -bparm;     /* Error out below */
            break;
        }
    }

    if (bparm < 2 || bparm > NUMSTR_MAXBASE) {
        nasm_nonfatal("invalid base %"PRId64" in %s()\n", bparm, s->name);
        return NULL;
    }

    if (dparm < -maxlen || dparm > maxlen) {
        nasm_nonfatal("digit count %"PRId64" specified to %s() too large",
                      dparm, s->name);
        dparm = -1;
    }

    /* Are we supposed to generate an empty string for zero? */
    if (!dparm && !n)
        decorate = 0;

    p = numbuf;
    *p++ = '\'';
    if (decorate) {
        *p++ = '0';
        *p++ = decorate;
    }

    p += numstr(p, maxlen, n, dparm, bparm, false);
    *p++ = '\'';
    *p = '\0';

    return new_Token(NULL, TOKEN_STR, numbuf, p - numbuf);
}

/* %abs() function */
static Token *
stdmac_abs(const SMacro *s, Token **params, int nparam)
{
    char numbuf[24];
    int len;
    int64_t v;
    uint64_t u;

    (void)s;
    (void)nparam;

    v = get_tok_num(params[0], NULL);
    u = v < 0 ? -v : v;

    /*
     * Don't use make_tok_num() here, to make sure we don't emit
     * a minus sign for the case of v = -2^63
     */
    len = snprintf(numbuf, sizeof numbuf, "%"PRIu64, u);
    return new_Token(NULL, TOKEN_NUM, numbuf, len);
}

/* %map() function */
static Token *
stdmac_map(const SMacro *s, Token **params, int nparam)
{
    const char *mname, *ctxname;
    SMacro *smac;
    Context *ctx;
    Token *t, *tline, *mstart;
    int i;
    Token *fixargs;
    int fixparams;              /* Number of fixed parameters */
    int mparams;                /* Number of variable parameters */
    int tparams;                /* Total number of parameters */
    int greedify;               /* Number of parameters that must be joined */
    Token **fparam;             /* Fixed parameters */
    Token **cparam;             /* Final list of macro call parameters */

    t = params[0];
    mname = get_id_noskip(&t, "%map");
    if (!mname)
        return NULL;

    mstart = t;

    fixargs = NULL;
    fixparams = 0;
    mparams = 1;
    t = skip_white(t->next);
    if (tok_is(t, ':')) {
        fixargs = t->next;
        fixparams = count_smacro_args(fixargs, &t);
        t = skip_white(t->next);

        if (tok_is(t, ':')) {
            struct ppscan pps;
            struct tokenval tokval;
            expr *evalresult;
            Token *ep;

            pps.tptr = ep = zap_white(expand_smacro_noreset(t->next));
            t->next = NULL;
            pps.ntokens = -1;
            tokval.t_type = TOKEN_INVALID;
            evalresult = evaluate(ppscan, &pps, &tokval, NULL, true, NULL);
            free_tlist(ep);

            if (!evalresult || tokval.t_type) {
                nasm_nonfatal("invalid expression in parameter count for `%s' in function %s",
                              mname, s->name);
                return NULL;
            } else if (!is_simple(evalresult)) {
                nasm_nonfatal("non-constant expression in parameter count for `%s' in function %s",
                              mname, s->name);
                return NULL;
            }
            mparams = reloc_value(evalresult);
            if (mparams < 1) {
                nasm_nonfatal("invalid parameter count for `%s' in function %s",
                              mname, s->name);
                return NULL;
            }
        }
    }

    nparam--;
    params++;
    if (nparam % mparams) {
        nasm_nonfatal("%s expected a multiple of %d expansion parameters, got %d\n",
                      s->name, mparams, nparam);
    }

    tparams = fixparams + mparams;

    ctx = get_ctx(mname, &ctxname);
    if (!smacro_defined(ctx, ctxname, tparams, &smac, true, false)
        || smac->nparam == 0 || (smac->in_progress && !smac->recursive)) {
        nasm_nonfatal("macro `%s' taking %d parameter%s not found in function %s",
                      mname, tparams, tparams == 1 ? "" : "s", s->name);
        return NULL;
    }

    if (nparam < mparams)
        return NULL;            /* Empty expansion */

    fparam = NULL;
    if (fixparams) {
        int nfp = fixparams;
        fparam = parse_smacro_args(&fixargs, &nfp, smac);
        if (nfp < fixparams) {
            fixparams = nfp;
            tparams = fixparams + mparams;
        }
    }

    greedify = 0;
    if (unlikely(tparams > smac->nparam)) {
        if (smac->params[smac->nparam-1].flags & SPARM_GREEDY)
            greedify = smac->nparam;
    }

    nasm_newn(cparam, tparams);

    tline = NULL;
    while (1) {
        int xparams;

        for (i = 0; i < fixparams; i++) {
            /* expand_smacro_with_params() is allowed to clobber the
             * parameter array, so we need to give it its own copy.
             */
            cparam[i] = dup_tlist(fparam[i], NULL);
        }

        for (i = fixparams; i < tparams; i++) {
            cparam[i] = *params;
            *params = NULL;     /* Taking over ownership */
            params++;
        }

        if (unlikely(greedify)) {
            /* Need to re-concatenate some number of arguments as
               comma-separated lists... */
            int i;
            Token **tp = &cparam[greedify-1];
            while (*tp)
                tp = &(*tp)->next;

            for (i = greedify; i < tparams; i++) {
                *tp = make_tok_char(NULL, ',');
                tp = steal_tlist(cparam[i], &(*tp)->next);
                cparam[i] = NULL;
            }
            xparams = greedify;
        } else {
            xparams = tparams;
        }

        t = expand_smacro_with_params(smac, mstart, cparam, xparams, NULL);
        if (t) {
            Token *rt = reverse_tokens(t);
            t->next = tline;
            tline = rt;
        }

        for (i = 0; i < xparams; i++)
            free_tlist(cparam[i]);

        nparam -= mparams;
        if (nparam < mparams)
            break;

        tline = make_tok_char(tline, ',');
    }

    nasm_free(fparam);
    nasm_free(cparam);

    return tline;
}

/* Add magic standard macros */
struct magic_macros {
    const char *name;
    bool casesense;
    int nparam;
    enum sparmflags flags;
    ExpandSMacro func;
};

static void pp_add_magic_stdmac(void)
{
    static const struct magic_macros magic_macros[] = {
        { "__?FILE?__", true, 0, 0, stdmac_file },
        { "__?LINE?__", true, 0, 0, stdmac_line },
        { "__?BITS?__", true, 0, 0, stdmac_bits },
        { "__?PTR?__",  true, 0, 0, stdmac_ptr },
        { "%abs",       false, 1, SPARM_EVAL, stdmac_abs },
        { "%count",     false, 1, SPARM_VARADIC, stdmac_count },
        { "%eval",      false, 1, SPARM_EVAL|SPARM_VARADIC, stdmac_join },
        { "%map",	false, 1, SPARM_VARADIC, stdmac_map },
        { "%str",       false, 1, SPARM_GREEDY|SPARM_STR, stdmac_join },
        { "%strcat",    false, 1, SPARM_STR|SPARM_CONDQUOTE|SPARM_VARADIC, stdmac_strcat },
        { "%strlen",    false, 1, SPARM_STR|SPARM_CONDQUOTE, stdmac_strlen },
        { "%tok",       false, 1, SPARM_STR|SPARM_CONDQUOTE, stdmac_tok },
        { NULL, false, 0, 0, NULL }
    };
    const struct magic_macros *m;
    SMacro tmpl;
    enum preproc_token pt;
    char name_buf[PP_TOKLEN_MAX+1];

    /*
     * Simple standard magic macros and functions.
     * Note that preprocessor functions are allowed to recurse.
     */
    nasm_zero(tmpl);
    for (m = magic_macros; m->name; m++) {
        tmpl.nparam = m->nparam;
        tmpl.expand = m->func;
        tmpl.recursive = m->nparam && m->name[0] == '%';

        if (m->nparam) {
            int i;
            enum sparmflags flags = m->flags;

            nasm_newn(tmpl.params, m->nparam);
            for (i = m->nparam-1; i >= 0; i--) {
                tmpl.params[i].flags = flags;
                /* These flags for the last arg only */
                flags &= ~(SPARM_GREEDY|SPARM_VARADIC|SPARM_OPTIONAL);
            }
        }
        define_smacro(m->name, m->casesense, NULL, &tmpl);
        if (m->name[0] == '%') {
            enum preproc_token op = pp_token_hash(m->name);
            if (op != PP_invalid)
                pp_op_may_be_function[op] = true;
        }
    }

    /* %hex() function */
    nasm_zero(tmpl);
    tmpl.nparam    = 1;
    tmpl.recursive = true;
    tmpl.expand    =  stdmac_join;
    nasm_newn(tmpl.params, tmpl.nparam);
    tmpl.params[0].flags = SPARM_EVAL|SPARM_UNSIGNED|SPARM_VARADIC;
    tmpl.params[0].radix = 'x';
    define_smacro("%hex", false, NULL, &tmpl);

    /* %sel() function */
    nasm_zero(tmpl);
    tmpl.nparam    = 2;
    tmpl.recursive = true;
    tmpl.expand    = stdmac_cond_sel;
    nasm_newn(tmpl.params, tmpl.nparam);
    tmpl.params[0].flags = SPARM_EVAL;
    tmpl.params[1].flags = SPARM_VARADIC;
    define_smacro("%sel", false, NULL, &tmpl);

    /* %cond() function, a variation on %sel */
    tmpl.nparam = 3;
    tmpl.expandpvt.u = 1;         /* Booleanize */
    nasm_newn(tmpl.params, tmpl.nparam);
    tmpl.params[0].flags = SPARM_EVAL;
    tmpl.params[1].flags = 0;
    tmpl.params[2].flags = SPARM_OPTIONAL;
    define_smacro("%cond", false, NULL, &tmpl);

    /* %num() function */
    nasm_zero(tmpl);
    tmpl.nparam = 3;
    tmpl.expand = stdmac_num;
    tmpl.recursive = true;
    nasm_newn(tmpl.params, tmpl.nparam);
    tmpl.params[0].flags = SPARM_EVAL;
    tmpl.params[1].flags = SPARM_EVAL|SPARM_OPTIONAL;
    tmpl.params[1].def   = make_tok_num(NULL, -1);
    tmpl.params[2].flags = SPARM_EVAL|SPARM_OPTIONAL;
    tmpl.params[2].def   = make_tok_num(NULL, 10);
    define_smacro("%num", false, NULL, &tmpl);

    /* %substr() function */
    nasm_zero(tmpl);
    tmpl.nparam = 3;
    tmpl.expand = stdmac_substr;
    tmpl.recursive = true;
    nasm_newn(tmpl.params, tmpl.nparam);
    tmpl.params[0].flags  = SPARM_STR|SPARM_CONDQUOTE;
    tmpl.params[1].flags  = SPARM_EVAL;
    tmpl.params[2].flags  = SPARM_EVAL|SPARM_OPTIONAL;
    tmpl.params[2].def    = make_tok_num(NULL, -1);
    define_smacro("%substr", false, NULL, &tmpl);

    /* %is...() macro functions */
    nasm_zero(tmpl);
    tmpl.nparam  = 1;
    tmpl.expand  = stdmac_is;
    tmpl.recursive = true;
    name_buf[0]  = '%';
    name_buf[1]  = 'i';
    name_buf[2]  = 's';
    for (pt = PP_IF; pt < (PP_IFN+(PP_IFN-PP_IF)); pt++) {
        if (!pp_directives[pt])
            continue;

        nasm_new(tmpl.params);
        tmpl.params[0].flags = SPARM_GREEDY;

        strcpy(name_buf+3, pp_directives[pt]+3);
        tmpl.expandpvt.u = pt;
        define_smacro(name_buf, false, NULL, &tmpl);
    }
}

static void pp_reset_stdmac(enum preproc_mode mode)
{
    int apass;
    struct Include *inc;

    /*
     * Set up the stdmac packages as a virtual include file,
     * indicated by a null file pointer.
     */
    nasm_new(inc);
    inc->next = istk;
    inc->nolist = inc->noline = !list_option('b');
    inc->where = istk->where;
    istk = inc;
    if (!istk->nolist) {
        lfmt->uplevel(LIST_INCLUDE, 0);
    }
    if (!istk->noline) {
        src_set(0, NULL);
        istk->where = src_where();
        if (ppdbg & PDBG_INCLUDE)
            dfmt->debug_include(true, istk->next->where, istk->where);
    }

    pp_add_magic_stdmac();

    if (tasm_compatible_mode)
        pp_add_stdmac(nasm_stdmac_tasm);

    pp_add_stdmac(nasm_stdmac_nasm);
    pp_add_stdmac(nasm_stdmac_version);

    if (extrastdmac)
        pp_add_stdmac(extrastdmac);

    stdmacpos  = stdmacros[0];
    stdmacnext = &stdmacros[1];

    do_predef = true;

    /*
     * Define the __?PASS?__ macro.  This is defined here unlike all the
     * other builtins, because it is special -- it varies between
     * passes -- but there is really no particular reason to make it
     * magic.
     *
     * 0 = dependencies only
     * 1 = preparatory passes
     * 2 = final pass
     * 3 = preprocess only
     */
    switch (mode) {
    case PP_NORMAL:
        apass = pass_final() ? 2 : 1;
        break;
    case PP_DEPS:
        apass = 0;
        break;
    case PP_PREPROC:
        apass = 3;
        break;
    default:
        panic();
    }

    define_smacro("__?PASS?__", true, make_tok_num(NULL, apass), NULL);
}

void pp_reset(const char *file, enum preproc_mode mode,
              struct strlist *dep_list)
{
    cstk = NULL;
    defining = NULL;
    nested_mac_count = 0;
    nested_rep_count = 0;
    init_macros();
    unique = 0;
    deplist = dep_list;
    pp_mode = mode;

    /* Reset options to default */
    nasm_zero(ppconf);

    /* Disable all debugging info, except in the last pass */
    ppdbg = 0;
    if (!(ppopt & PP_TRIVIAL)) {
        if (pass_final()) {
            if (dfmt->debug_mmacros)
                ppdbg |= PDBG_MMACROS;
            if (dfmt->debug_smacros)
                ppdbg |= PDBG_SMACROS;
            if (dfmt->debug_include)
                ppdbg |= PDBG_INCLUDE;
        }

        if (list_option('s'))
            ppdbg |= PDBG_LIST_SMACROS;
    }

    memset(use_loaded, 0, use_package_count * sizeof(bool));

    /* First set up the top level input file */
    nasm_new(istk);
    istk->fp = nasm_open_read(file, NF_TEXT);
    if (!istk->fp) {
	nasm_fatalf(ERR_NOFILE, "unable to open input file `%s'%s%s",
                    file, errno ? " " : "", errno ? strerror(errno) : "");
    }
    src_set(0, file);
    istk->where = src_where();
    istk->lineinc = 1;

    if (ppdbg & PDBG_INCLUDE) {
        /* Let the debug format know the main file */
        dfmt->debug_include(true, src_nowhere(), istk->where);
    }

    strlist_add(deplist, file);

    do_predef = false;

    if (!(ppopt & PP_TRIVIAL))
        pp_reset_stdmac(mode);
}

void pp_init(enum preproc_opt opt)
{
    ppopt = opt;
    nasm_newn(use_loaded, use_package_count);
}

/*
 * Get a line of tokens. If we popped the macro expansion/include stack,
 * we return a pointer to the dummy token tok_pop; at that point if
 * istk is NULL then we have reached end of input;
 */
static Token tok_pop;           /* Dummy token placeholder */

static Token *pp_tokline(void)
{
    while (true) {
        Line *l = istk->expansion;
        Token *tline = NULL;
        Token *dtline;

        /*
         * Fetch a tokenized line, either from the macro-expansion
         * buffer or from the input file.
         */
        tline = NULL;
        while (l && l->finishes) {
            MMacro *fm = l->finishes;

            nasm_assert(fm == istk->mstk.mstk);

            if (!fm->name && fm->in_progress > 1) {
                /*
                 * This is a macro-end marker for a macro with no
                 * name, which means it's not really a macro at all
                 * but a %rep block, and the `in_progress' field is
                 * more than 1, meaning that we still need to
                 * repeat. (1 means the natural last repetition; 0
                 * means termination by %exitrep.) We have
                 * therefore expanded up to the %endrep, and must
                 * push the whole block on to the expansion buffer
                 * again. We don't bother to remove the macro-end
                 * marker: we'd only have to generate another one
                 * if we did.
                 */
                fm->in_progress--;
                list_for_each(l, fm->expansion) {
                    Line *ll;

                    nasm_new(ll);
                    ll->next  = istk->expansion;
                    ll->first = dup_tlist(l->first, NULL);
                    ll->where = l->where;
                    istk->expansion = ll;
                }
                l = istk->expansion;
                continue;
            } else {
                MMacro *m = istk->mstk.mstk;

                /*
                 * Check whether a `%rep' was started and not ended
                 * within this macro expansion. This can happen and
                 * should be detected. It's a fatal error because
                 * I'm too confused to work out how to recover
                 * sensibly from it.
                 */
                if (defining) {
                    if (defining->name)
                        nasm_panic("defining with name in expansion");
                    else if (m->name)
                        nasm_fatal("`%%rep' without `%%endrep' within"
				   " expansion of macro `%s'", m->name);
                }

                /*
                 * FIXME:  investigate the relationship at this point between
                 * istk->mstk.mstk and fm
                 */
                istk->mstk = m->mstk;
                if (m->name) {
                    /*
                     * This was a real macro call, not a %rep, and
                     * therefore the parameter information needs to
                     * be freed and the iteration count/nesting
                     * depth adjusted.
                     */

                    if (!--mmacro_deadman.levels) {
                        /*
                         * If all mmacro processing done,
                         * clear all counters and the deadman
                         * message trigger.
                         */
                        nasm_zero(mmacro_deadman); /* Clear all counters */
                    }

#if 0
                    if (m->prev) {
                        pop_mmacro(m);
                        fm->in_progress --;
                    } else
#endif
                    {
                        nasm_free(m->params);
                        nasm_free(m->iname);
                        free_tlist(m->iline);
                        nasm_free(m->paramlen);
                        fm->in_progress = 0;
			m->params = NULL;
                        m->iname = NULL;
			m->iline = NULL;
			m->paramlen = NULL;
                    }
                }

                if (fm->nolist & NL_LINE) {
                    istk->noline--;
                } else if (!istk->noline) {
                    if (fm == src_macro_current())
                        src_macro_pop();
                    src_update(l->where);
                }

                if (fm->nolist & NL_LIST) {
                    istk->nolist--;
                } else if (!istk->nolist) {
                    lfmt->downlevel(LIST_MACRO);
                    if ((ppdbg & PDBG_MMACROS) && fm->name)
                        debug_macro_end(fm);
                }

                istk->where = l->where;

                if (!m->name)
                    free_mmacro(m);
            }
            istk->expansion = l->next;
            nasm_free(l);

            return &tok_pop;
        }

        do {                    /* until we get a line we can use */
            char *line;

            if (istk->expansion) {      /* from a macro expansion */
                Line *l = istk->expansion;

                istk->expansion = l->next;
                istk->where = l->where;
                tline = l->first;
                nasm_free(l);

                if (!istk->noline)
                    src_update(istk->where);

                if (!istk->nolist) {
                    line = detoken(tline, false);
                    lfmt->line(LIST_MACRO, istk->where.lineno, line);
                    nasm_free(line);
                }
            } else if ((line = read_line())) {
                tline = tokenize(line);
                nasm_free(line);
            } else {
                /*
                 * The current file has ended; work down the istk
                 */
                Include *i = istk;

                if (i->fp)
                    fclose(i->fp);
                if (i->conds) {
                    /* nasm_fatal can't be conditionally suppressed */
                    nasm_fatal("expected `%%endif' before end of file");
                }

                istk = i->next;

                if (!i->nolist)
                    lfmt->downlevel(LIST_INCLUDE);
                if (!i->noline) {
                    struct src_location whereto
                        = istk ? istk->where : src_nowhere();
                    if (ppdbg & PDBG_INCLUDE)
                        dfmt->debug_include(false, whereto, i->where);
                    if (istk)
                        src_update(istk->where);
                }

                nasm_free(i);
                return &tok_pop;
            }
        } while (0);

        /*
         * We must expand MMacro parameters and MMacro-local labels
         * _before_ we plunge into directive processing, to cope
         * with things like `%define something %1' such as STRUC
         * uses. Unless we're _defining_ a MMacro, in which case
         * those tokens should be left alone to go into the
         * definition; and unless we're in a non-emitting
         * condition, in which case we don't want to meddle with
         * anything.
         */
        if (!defining &&
            !(istk->conds && !emitting(istk->conds->state)) &&
            !(istk->mstk.mmac && !istk->mstk.mmac->in_progress)) {
            tline = expand_mmac_params(tline);
        }

        /*
         * Check the line to see if it's a preprocessor directive.
         */
        if (do_directive(tline, &dtline) == DIRECTIVE_FOUND) {
            if (dtline)
                return dtline;
        } else if (defining) {
            /*
             * We're defining a multi-line macro. We emit nothing
             * at all, and just
             * shove the tokenized line on to the macro definition.
             */
            MMacro *mmac = defining->dstk.mmac;
            Line *l;

            nasm_new(l);
            l->next = defining->expansion;
            l->first = tline;
            l->finishes = NULL;
            l->where = istk->where;
            defining->expansion = l;

            /*
             * Remember if this mmacro expansion contains %00:
             * if it does, we will have to handle leading labels
             * specially.
             */
            if (mmac) {
                const Token *t;
                list_for_each(t, tline) {
                    if (t->type == TOKEN_MMACRO_PARAM &&
                        !memcmp(t->text.a, "%00", 4))
                        mmac->capture_label = true;
                }
            }
        } else if (istk->conds && !emitting(istk->conds->state)) {
            /*
             * We're in a non-emitting branch of a condition block.
             * Emit nothing at all, not even a blank line: when we
             * emerge from the condition we'll give a line-number
             * directive so we keep our place correctly.
             */
            free_tlist(tline);
        } else if (istk->mstk.mstk && !istk->mstk.mstk->in_progress) {
            /*
             * We're in a %rep block which has been terminated, so
             * we're walking through to the %endrep without
             * emitting anything. Emit nothing at all, not even a
             * blank line: when we emerge from the %rep block we'll
             * give a line-number directive so we keep our place
             * correctly.
             */
            free_tlist(tline);
        } else {
            tline = expand_smacro(tline);
            if (!expand_mmacro(tline))
                return tline;
        }
    }
}

char *pp_getline(void)
{
    char *line = NULL;
    Token *tline;

    while (true) {
        tline = pp_tokline();
        if (tline == &tok_pop) {
            /*
             * We popped the macro/include stack. If istk is empty,
             * we are at end of input, otherwise just loop back.
             */
            if (!istk)
                break;
        } else {
            /*
             * De-tokenize the line and emit it.
             */
            line = detoken(tline, true);
            free_tlist(tline);
            break;
        }
    }

    if (list_option('e') && istk && !istk->nolist && line && line[0]) {
        char *buf = nasm_strcat(" ;;; ", line);
        lfmt->line(LIST_MACRO, -1, buf);
        nasm_free(buf);
    }

    return line;
}

void pp_cleanup_pass(void)
{
    if (defining) {
        if (defining->name) {
            nasm_nonfatal("end of file while still defining macro `%s'",
                          defining->name);
        } else {
            nasm_nonfatal("end of file while still in %%rep");
        }

        free_mmacro(defining);
        defining = NULL;
    }

    while (cstk)
        ctx_pop();
    free_macros();
    while (istk) {
        Include *i = istk;
        istk = istk->next;
        fclose(i->fp);
        if (!istk && (ppdbg & PDBG_INCLUDE)) {
            /* Signal closing the top-level input file */
            dfmt->debug_include(false, src_nowhere(), i->where);
        }
        nasm_free(i);
    }
    while (cstk)
        ctx_pop();
    src_set_fname(NULL);

    if (ppdbg & PDBG_MMACROS)
        debug_macro_output();
}

void pp_cleanup_session(void)
{
    nasm_free(use_loaded);
    free_llist(predef);
    predef = NULL;
    delete_Blocks();
    ipath_list = NULL;
}

void pp_include_path(struct strlist *list)
{
    ipath_list = list;
}

void pp_pre_include(char *fname)
{
    Token *inc, *space, *name;
    Line *l;

    name = new_Token(NULL, TOKEN_INTERNAL_STR, fname, 0);
    space = new_White(name);
    inc = new_Token(space, TOKEN_PREPROC_ID, "%include", 0);

    l = nasm_malloc(sizeof(Line));
    l->next = predef;
    l->first = inc;
    l->finishes = NULL;
    predef = l;
}

void pp_pre_define(char *definition)
{
    Token *def, *space;
    Line *l;
    char *equals;

    equals = strchr(definition, '=');
    space = new_White(NULL);
    def = new_Token(space, TOKEN_PREPROC_ID, "%define", 0);
    if (equals)
        *equals = ' ';
    space->next = tokenize(definition);
    if (equals)
        *equals = '=';

    nasm_new(l);
    l->next = predef;
    l->first = def;
    l->finishes = NULL;
    predef = l;
}

void pp_pre_undefine(char *definition)
{
    Token *def, *space;
    Line *l;

    space = new_White(NULL);
    def = new_Token(space, TOKEN_PREPROC_ID, "%undef", 0);
    space->next = tokenize(definition);

    nasm_new(l);
    l->next = predef;
    l->first = def;
    l->finishes = NULL;
    predef = l;
}

/* Insert an early preprocessor command that doesn't need special handling */
void pp_pre_command(const char *what, char *string)
{
    Token *def, *space;
    Line *l;

    def = tokenize(string);
    if (what) {
        space = new_White(def);
        def = new_Token(space, TOKEN_PREPROC_ID, what, 0);
    }

    nasm_new(l);
    l->next = predef;
    l->first = def;
    l->finishes = NULL;
    predef = l;
}

static void pp_add_stdmac(macros_t *macros)
{
    macros_t **mp;

    /* Find the end of the list and avoid duplicates */
    for (mp = stdmacros; *mp; mp++) {
        if (*mp == macros)
            return;             /* Nothing to do */
    }

    nasm_assert(mp < &stdmacros[ARRAY_SIZE(stdmacros)-1]);

    *mp = macros;
}

void pp_extra_stdmac(macros_t *macros)
{
        extrastdmac = macros;
}

/* Create a numeric token, with possible - token in front */
static Token *make_tok_num(Token *next, int64_t val)
{
    char numbuf[32];
    int len;
    uint64_t uval;
    bool minus = val < 0;

    uval = minus ? -val : val;

    len = snprintf(numbuf, sizeof numbuf, "%"PRIu64, uval);
    next = new_Token(next, TOKEN_NUM, numbuf, len);

    if (minus)
        next = make_tok_char(next, '-');

    return next;
}

/*
 * Create a numeric token with specified radix and signedness;
 * prefix the number with 0<radix> if a radix letter is specified,
 * otherwise generate a decimal constant without prefix.
 */
static Token *
make_tok_num_radix(Token *next, int64_t val, char radix, bool uns)
{
    char numbuf[2+64+1]; /* Maximum possible: 0b + binary + null */
    char *p;
    uint64_t uval;
    bool minus = val < 0 && !uns;
    unsigned int base;
    bool upper;

    uval = minus ? -val : val;

    p = numbuf;
    base = 10;
    upper = false;
    if (radix) {
        *p++ = '0';
        *p++ = radix;

        base = radix_letter(radix);
        upper = !(radix & 0x20);
    }

    p += numstr(p, 64, uval, -1, base, upper);
    next = new_Token(next, TOKEN_NUM, numbuf, p - numbuf);

    if (minus)
        next = make_tok_char(next, '-');

    return next;
}

/*
 * Do the inverse of make_tok_num(). This only needs to be able
 * to parse the output of make_tok_num() or make_tok_hex().
 */
static int64_t get_tok_num(const Token *t, bool *err)
{
    bool minus = false;
    int64_t v;

    if (tok_is(t, '-')) {
        minus = true;
        t = t->next;
    }
    if (!tok_is(t, TOKEN_NUM)) {
        if (err)
            *err = true;
        return 0;
    }

    v = readnum(tok_text(t), err);
    return minus ? -v : v;
}

/* Create a quoted string token */
static Token *make_tok_qstr_len(Token *next, const char *str, size_t len)
{
    char *p = nasm_quote(str, &len);
    return new_Token_free(next, TOKEN_STR, p, len);
}
static Token *make_tok_qstr(Token *next, const char *str)
{
    return make_tok_qstr_len(next, str, strlen(str));
}

/* Create a single-character operator token */
static Token *make_tok_char(Token *next, char op)
{
    Token *t = new_Token(next, op, NULL, 1);
    t->text.a[0] = op;
    return t;
}

/*
 * Descent the macro hierarchy and display the expansion after
 * encountering an error message.
 */
void pp_error_list_macros(errflags severity)
{
    const MMacro *m;

    severity |= ERR_PP_LISTMACRO | ERR_NO_SEVERITY | ERR_HERE;

    while ((m = src_error_down())) {
        if ((m->nolist & NL_LIST) || !m->where.filename)
            break;
	nasm_error(severity, "... from macro `%s' defined", m->name);
    }

    src_error_reset();
}
