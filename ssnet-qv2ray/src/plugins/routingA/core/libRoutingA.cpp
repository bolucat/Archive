#include "libRoutingA.hpp"

#include <QDebug>
#include <QObject>
#include <QStack>

namespace RoutingA
{
    QPair<QList<Defination>, QList<Routing>> ParseRoutingA(const QString &program)
    {
        const auto syms = _details::GenerateSyntaxTree(program);
        const auto rasyms = _details::ParseS(syms);

        QList<Defination> defines;
        QList<Routing> routings;

        for (const auto &token : rasyms)
        {
            if (token.children.length() != 1)
                throw ParsingErrorException(QObject::tr("Unexpected empty tree."), 0, token.value);
            else if (const auto sym = token.children.first().sym; sym == _details::RA_Symbol::_B)
                defines << ParseDefination(token.children[0]);
            else if (sym == _details::RA_Symbol::_C)
                routings << ParseRouting(token.children[0]);
            else
                throw ParsingErrorException(QObject::tr("Unexpected node symbol."), 0, token.value);
        }

        return { defines, routings };
    }
} // namespace RoutingA

namespace RoutingA::_details
{
    const static inline QMap<QChar, RA_Symbol> char_header_map{
        // clang-format off
        { ',',  RA_Symbol::comma },
        { '\'', RA_Symbol::single_quote },
        { '"',  RA_Symbol::double_quote },
        { '(',  RA_Symbol::left_parenthesis },
        { ')',  RA_Symbol::right_parenthesis },
        { ':',  RA_Symbol::colon },
        { '&',  RA_Symbol::andsign },
        { '-',  RA_Symbol::minussign },
        { '>',  RA_Symbol::greatersign },
        { '=',  RA_Symbol::equalsign },
        { '\n', RA_Symbol::newline },
        // clang-format on
    };

    QString Preprocess(const QString &prog)
    {
        QStringList lines;
        for (auto &s : prog.split('\n'))
            if (s = s.trimmed(); !s.startsWith('#') && !s.isEmpty())
                lines << s;
        return lines.join('\n');
    }

    bool SkipSpaces(QChar c, int stackTop)
    {
        const static std::initializer_list<int> slist{ 0, 12, 15, 22, 27, 55, 37, 18, 25, 20, 50, 44, 52 };
        if (c.isSpace() && c != '\n')
            if (std::find(slist.begin(), slist.end(), stackTop) != slist.end())
                return true;
        return false;
    }

    RA_Symbol GetSymbol(QChar c)
    {
        // 45	o-> , ' " ( )
        // const static QList<char> o{ ',', '\'', '"', '(', ')' };
        const static QList<char> reserved{ ',', '\'', '"', '(', ')', ':', '&', '-', '>', '=' };

        if (c.isPunct())
        {
            if (c == '_')
                return RA_Symbol::_k;
            else
                return reserved.contains(c) ? char_header_map.value(c) : RA_Symbol::_n;
        }
        else if (c.isDigit())
            return RA_Symbol::digit;
        else if (c == '\n')
            return char_header_map.value(c);
        else if (c.isNull())
            return RA_Symbol::end;
        else if (reserved.contains(c))
            return char_header_map.value(c);
        return RA_Symbol::_k;
    }

    void ProcessSpecialCases(RA_Action &item, const QString &str, int i)
    {
        if (item.actionType != RA_Action::Special1)
            return;

        for (auto j = i + 1; j < str.length(); j++)
        {
            if (str[j] == ':')
            {
                item.actionType = RA_Action::R, item.state = 11;
                return;
            }
            if (str[j] == ',' || str[j] == ')')
            {
                item.actionType = RA_Action::S, item.state = 44;
                return;
            }
        }
        throw ParsingErrorException(QObject::tr("Unexpected special case unmatched."), 0, u""_qs);
    }

    RA_Token GenerateSyntaxTree(const QString &prog)
    {
        QStack<RA_Token> stackR;
        QStack<int> states;
        stackR.push({});
        states.push(0);

        const auto program = Preprocess(prog) + char_header_map.key(stackR.top().sym);

        const auto getLine = [&](int i)
        {
            auto lineStartPos = program.lastIndexOf('\n', i);
            auto lineEndPos = program.indexOf('\n', i);

            if (lineStartPos == -1)
                lineStartPos = 0;

            if (lineEndPos == -1)
                lineEndPos = program.length();

            return program.sliced(lineStartPos, lineEndPos - lineStartPos);
        };

        QList<RA_Token> reducedSyms;
        for (auto i = 0; i < program.length();)
        {
            const auto sTop = states.top();
            if (SkipSpaces(program[i], sTop))
            {
                i++;
                continue;
            }

            const auto sym = GetSymbol(program[i]);
            auto msg = GetAction(sTop, sym);
            ProcessSpecialCases(msg, program, i);

            switch (msg.actionType)
            {
                case RA_Action::S:
                {
                    states.push(msg.state);
                    stackR.push(RA_Token{ sym, program[i] });
                    i++;
                    break;
                }
                case RA_Action::R:
                {
                    const auto production = GetProduction(msg.state);
                    const auto rightLen = strlen(production.right);

                    for (size_t _ = 0; _ < rightLen; _++)
                        states.pop();

                    const auto gt = GetAction(states.top(), production.symbol);

                    assert(gt.actionType == RA_Action::atype::Nul);
                    states.push(gt.state);

                    reducedSyms.clear();
                    const auto sz = stackR.size();
                    for (qsizetype ii = stackR.size() - rightLen; ii < sz; ii++)
                        reducedSyms.prepend(stackR.takeLast());

                    QString value;
                    for (const auto &syms : reducedSyms)
                        value.append(syms.value);

                    stackR.push(RA_Token{ production.symbol, value, reducedSyms });
                    break;
                }
                case RA_Action::Accept:
                {
                    return stackR.pop();
                }
                case RA_Action::Nul:
                {
                    const auto line = getLine(i);
                    if (i == program.length() - 1)
                        throw ParsingErrorException("Unexpected EOF", i, line);

                    const auto lineStartPos = program.lastIndexOf('\n', i);
                    throw ParsingErrorException(line + '\n' + u" "_qs.repeated(i - lineStartPos - 1) + "^ unexpected char here.", i, line);
                }
                default: throw ParsingErrorException(u"Unreachable condition reached."_qs, i, getLine(i));
            }
        }

        return {};
    }

    Routing ParseRouting(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_C || !SymbolMatches(t.children, { RA_Symbol::_F, RA_Symbol::_Q, RA_Symbol::minussign, RA_Symbol::greatersign, RA_Symbol::_D }))
            return {};

        Routing r;
        r.outboundTag = t.children[4].value.trimmed();
        r.rules << ParseFunction(t.children[0]);
        r.rules << ParseQ(t.children[1]);
        return r;
    }

    QList<Function> ParseQ(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_Q)
            return {};

        QList<Function> functions;
        if (SymbolMatches(t.children, { RA_Symbol::andsign, RA_Symbol::andsign, RA_Symbol::_F, RA_Symbol::_Q }))
        {
            functions << ParseFunction(t.children[2]);
            functions << ParseQ(t.children[3]);
        }
        return functions;
    }

    Defination ParseDefination(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_B || !SymbolMatches(t.children, { RA_Symbol::_D, RA_Symbol::colon, RA_Symbol::_E }))
            return {};

        const auto E = t.children[2];
        Defination d;
        d.type = t.children[0].value;

        if (SymbolMatches(E.children, { RA_Symbol::_D, RA_Symbol::equalsign, RA_Symbol::_F }))
            d.content = ParseIOBound(E);
        else if (SymbolMatches(E.children, { RA_Symbol::_D }))
            d.value = E.children[0].value;

        return d;
    }

    DefinationContent ParseIOBound(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_E || !SymbolMatches(t.children, { RA_Symbol::_D, RA_Symbol::equalsign, RA_Symbol::_F }))
            return {};
        DefinationContent b;
        b.name = t.children[0].value.trimmed();
        b.function = ParseFunction(t.children[2]);
        return b;
    }

    Function ParseFunction(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_F)
            return {};
        Function f;
        f.name = t.children[0].value;
        std::tie(f.params, f.namedParams) = parseG(t.children[2]);
        return f;
    }

    QList<RA_Token> ParseS(const RA_Token &s)
    {
        if (s.sym != RA_Symbol::_S)
            return {};

        QList<RA_Token> t;
        t.append(s.children[0]);
        t.append(ParseR(s.children[1]));
        return t;
    }

    QList<RA_Token> ParseR(const RA_Token &s)
    {
        if (s.sym != RA_Symbol::_R)
            return {};

        QList<RA_Token> t;
        if (SymbolMatches(s.children, { RA_Symbol::newline, RA_Symbol::_A, RA_Symbol::_R }))
            t.append(s.children[1]), t.append(ParseR(s.children[2]));

        return t;
    }

    std::pair<QStringList, QMap<QString, QStringList>> parseG(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_G)
            return {};
        QStringList params;
        QMap<QString, QStringList> namedParams;

        if (SymbolMatches(t.children, { RA_Symbol::_H, RA_Symbol::_M, RA_Symbol::_N }))
        {
            params.append(ParseH(t.children[0]));
            params.append(ParseM(t.children[1]));
            namedParams.insert(ParseN(t.children[2]));
        }
        else if (SymbolMatches(t.children, { RA_Symbol::_H, RA_Symbol::colon, RA_Symbol::_H, RA_Symbol::_N }))
        {
            namedParams.insert(ParseHHN(t));
        }
        return std::pair{ params, namedParams };
    }

    QMap<QString, QStringList> ParseHHN(const RA_Token &t)
    {
        if (!SymbolMatches(t.children, { RA_Symbol::_H, RA_Symbol::colon, RA_Symbol::_H, RA_Symbol::_N }))
            return {};
        QMap<QString, QStringList> result;
        result[t.children[0].value].append(ParseH(t.children[2]));
        const auto n = t.children[3];
        if (!n.children.isEmpty())
            for (const auto &[k, v] : ParseN(n).toStdMap())
                result[k].append(v);
        return result;
    }

    QMap<QString, QStringList> ParseN(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_N)
            return {};
        if (!SymbolMatches(t.children, { RA_Symbol::comma, RA_Symbol::_H, RA_Symbol::colon, RA_Symbol::_H, RA_Symbol::_N }))
            return {};
        return ParseHHN(t.sliced(1));
    }

    QStringList ParseM(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_M)
            return {};
        if (t.children.isEmpty())
            return {};
        QStringList result;
        result << ParseH(t.children[1]);
        result << ParseM(t.children[2]);
        return result;
    }

    QString ParseH(const RA_Token &t)
    {
        if (t.sym != RA_Symbol::_H)
            return {};
        if ((t.value.startsWith('\'') && t.value.endsWith('\'')) || (t.value.startsWith('"') && t.value.endsWith('"')))
            return t.value.sliced(1, t.value.length() - 2);
        return t.value;
    }

    bool SymbolMatches(const QList<RA_Token> &symbols, const QList<RA_Symbol> &syms)
    {
        if (symbols.length() != syms.length())
            return false;
        for (auto i = 0; i < symbols.length(); i++)
            if (symbols[i].sym != syms[i])
                return false;
        return true;
    }

} // namespace RoutingA::_details
