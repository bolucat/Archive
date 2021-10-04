#pragma once

#include <QException>
#include <QList>
#include <QMap>
#include <QMetaType>
#include <QString>

namespace RoutingA
{
    struct Function
    {
        QString name;
        QStringList params;
        QMap<QString, QStringList> namedParams;
    };

    struct DefinationContent
    {
        QString name;
        Function function;
    };

    struct Defination
    {
        QString type;
        QString value;
        DefinationContent content;
    };

    struct Routing
    {
        QList<Function> rules;
        QString outboundTag;
    };

    class ParsingErrorException
    {
      public:
        ParsingErrorException(const QString &s, int pos, const QString &line) : Message(s.trimmed()), Position(pos), Line(line.trimmed()){};
        QString Message;
        int Position;
        QString Line;
    };

    QPair<QList<Defination>, QList<Routing>> ParseRoutingA(const QString &program);

    namespace _details
    {
        enum class RA_Symbol
        {
            // clang-format off
            comma, single_quote, double_quote, left_parenthesis, right_parenthesis, colon, newline,
            _k, digit, _n,
            end, andsign, minussign, greatersign, equalsign,
            _A, _B, _C, _D, _E, _F, _G, _H, _I, _J, _K, _L, _M, _N, _O, _P, _Q, _R, _S, _T, _U, _V, _W, _X, _Y, _Z,
            // clang-format on
            Nul
        };

        struct RA_Action
        {
            enum atype
            {
                // clang-format off
            Nul, S, R, Num, Special1, Accept,
                // clang-format on
            };

            constexpr RA_Action(int v) : actionType(Nul), state(v){};
            constexpr RA_Action(atype t = Nul, int v = 0) : actionType(t), state(v){};

            atype actionType;
            int state;
        };

        struct RA_Production
        {
            RA_Symbol symbol;
            const char *const right;
        };

        struct RA_Token
        {
            RA_Symbol sym = RA_Symbol::Nul;
            QList<RA_Token> children;
            QString value;

            RA_Token(){};
            RA_Token(RA_Symbol h, const QString &v, const QList<RA_Token> &c = {}) : sym(h), children(c), value(v){};

            RA_Token sliced(int from) const
            {
                RA_Token t = *this;
                t.children = t.children.sliced(from);
                return t;
            }
        };

        RA_Action GetAction(int row, RA_Symbol column) noexcept;
        RA_Production GetProduction(int) noexcept;

        QString Preprocess(const QString &prog);
        bool SkipSpaces(QChar c, int stackTop);
        RA_Symbol GetSymbol(QChar c);
        void ProcessSpecialCases(RA_Action &item, const QString &str, int i);
        RA_Token GenerateSyntaxTree(const QString &prog);

        bool SymbolMatches(const QList<RA_Token> &symbols, const QList<RA_Symbol> &syms);
        std::pair<QStringList, QMap<QString, QStringList>> parseG(const RA_Token &t);
        QString ParseH(const RA_Token &t);
        QMap<QString, QStringList> ParseHHN(const RA_Token &t);
        QStringList ParseM(const RA_Token &t);
        QMap<QString, QStringList> ParseN(const RA_Token &t);
        QList<Function> ParseQ(const RA_Token &t);
        QList<RA_Token> ParseR(const RA_Token &s);
        QList<RA_Token> ParseS(const RA_Token &s);

        Function ParseFunction(const RA_Token &t);
        DefinationContent ParseIOBound(const RA_Token &t);
        Defination ParseDefination(const RA_Token &t);
        Routing ParseRouting(const RA_Token &t);
    } // namespace _details
} // namespace RoutingA

Q_DECLARE_METATYPE(RoutingA::Defination)
Q_DECLARE_METATYPE(RoutingA::DefinationContent)
Q_DECLARE_METATYPE(RoutingA::Function)
