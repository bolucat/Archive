#include "../src/PureJson.hpp"

#include <iostream>
#include <list>
#include <string>
//
#define ASSERT_EQUAL(string1, string2) assert((string1) == (string2))
//
//
std::list<std::string> tests{};
std::list<std::string> expects{}; // TODO
//
void begin_test()
{
    for (auto &str : tests)
    {
        std::cout << "source: " << str << std::endl;
        std::cout << "target: " << RemoveComment(str) << std::endl;
    }
}
//
int main()
{
    // Simple inline comments.
    tests.push_back(std::string(R"(   //this is a comment   )"));
    tests.push_back(std::string(R"(   this is not a comment   )"));
    // Somewhat more complex comments within and out of the qoutes.
    tests.push_back(std::string(R"(   "//this is not a comment, it's in the string"   )"));
    tests.push_back(
        std::string(R"(   "//this is not a comment, it's in the string", but //those are comments to be removed.   )"));
    // More complex comments with fake qoutes (escaped)
    tests.push_back(
        std::string(R"(   "//this is not a comment, it's in the string \", and //those are not comments neither"   )"));
    tests.push_back(std::string(
        R"(   "//this is not a comment, it's in the string \\", but //those are comments since the string is terminated   )"));
    // Test cases with single and double qoutes.
    tests.push_back(std::string(
        R"(   '//this is not a comment, it's in the string, // but, only for the first part and those are comments since the string is terminated   )"));
    tests.push_back(std::string(
        R"(   "//this is not a comment, it's in the string ", and '//those are not comments as well' since in the //single qoutes."   )"));
    // Test cases with block comments.
    tests.push_back(std::string(R"(   /*this is a comment*/ my actrual data   )"));
    tests.push_back(std::string(R"(   /**/ my actrual data /**/  )"));
    tests.push_back(std::string(R"(   /**/ my actr/**/ual data /**/  )"));
    tests.push_back(std::string(R"(   /**/ my actr/****////**/**///**/**/ual data /**/  )"));
    // Test cases with blocked comments single and double qoutes.
    tests.push_back(std::string(
        R"(   '//this is not a comment, it's i/**/n the st/**/ring, // but, on/**/ly for the first part and those are comments since the string is terminated   )"));
    tests.push_back(std::string(
        R"(   "//this is not a comment, it's /*in the string*/ ", and '//those are not comments as well' since in the single qoutes./* and in the comments */"   )"));
    //
    begin_test();
    return 0;
}
