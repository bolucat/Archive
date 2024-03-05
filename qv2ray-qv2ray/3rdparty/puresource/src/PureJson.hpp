#pragma once
#ifndef HAS_PUREJSON_HPP
    #define HAS_PUREJSON_HPP
    #include <sstream>
    #include <string>
    #include <string_view>
    #if !defined(_WIN32) && (defined(__unix__) || defined(__unix) || (defined(__APPLE__) && defined(__MACH__)))
        #define EOL_STRING "\n"
    #else
        #define EOL_STRING "\r\n"
    #endif

std::string RemoveComment(const std::string &source)
{
    std::istringstream source_stream(source);
    std::string targetText = "";
    bool isInBlockComment = false;
    std::string text;

    while (std::getline(source_stream, text))
    {
        bool isInLineComment = false;
        bool isInDoubleQoute = false;
        bool isInSingleQoute = false;
        std::string currentLineParsed;
        //
        // 0 = not a fslash, 1 = first one. 2 = second one.
        bool willNextEscape = false;

        for (size_t i = 0; i < text.length(); i++)
        {
            if (isInLineComment)
                continue;

            const auto current = text.at(i);
            const auto priv = (i == 0) ? '\0' : text.at(i - 1);
            const auto next = (i == text.length() - 1) ? '\0' : text.at(i + 1);
            //
            bool currentIsBeingEscaped = false;

            // If the next char will not be escape
            if (!willNextEscape)
            {
                // but find a metachar....
                if (current == '\\')
                {
                    // Meaning the next char will be escaped.
                    willNextEscape = true;
                }
            }
            else
            {
                // the current char will be escaped.
                willNextEscape = false;
                currentIsBeingEscaped = true;
            }

            // Not in the comment.
            if (!isInBlockComment && !isInLineComment)
            {
                // If the current one will NOT be escaped and it's a qoute. Like \" and \'
                if (!currentIsBeingEscaped && !isInSingleQoute && (current == '"'))
                {
                    isInDoubleQoute = !isInDoubleQoute;
                }
                if (!currentIsBeingEscaped && !isInDoubleQoute && (current == '\''))
                {
                    isInSingleQoute = !isInSingleQoute;
                }
            }

            if (!isInDoubleQoute && !isInSingleQoute)
            {
                // First try to close the block comment.
                if (current == '/' && priv == '*')
                {
                    isInBlockComment = false;
                    continue;
                }

                if (current == '/' && next == '/')
                {
                    isInLineComment = true;
                    continue;
                }

                if (current == '/' && next == '*')
                {
                    isInBlockComment = true;
                    continue;
                }
            }

            if (!isInLineComment && !isInBlockComment)
            {
                currentLineParsed.push_back(current);
                // Reset escape char state.
            }
        }

        if (!currentLineParsed.empty())
        {
            targetText.append(currentLineParsed + EOL_STRING);
        }
    }

    return targetText;
}

    #ifdef QT_CORE_LIB
        #include <QString>
QString RemoveComment(const QString &str)
{
    return QString::fromStdString(RemoveComment(str.toStdString()));
}
    #endif
#endif // HAS_PUREJSON_HPP
