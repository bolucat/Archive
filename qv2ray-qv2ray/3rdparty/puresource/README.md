# PureSource
A C++ helper library to remove comments from any C style source codes.

## Examples
- Please check the `examples` folder for usage in Qt or check the [test file](tests/main.cpp)

```c++
source:    //this is a comment   
target:    

source:    this is not a comment   
target:    this is not a comment   

source:    "//this is not a comment, it's in the string"   
target:    "//this is not a comment, it's in the string"   

source:    "//this is not a comment, it's in the string", but //those are comments to be removed.   
target:    "//this is not a comment, it's in the string", but 

source:    "//this is not a comment, it's in the string \", and //those are not comments neither"   
target:    "//this is not a comment, it's in the string \", and //those are not comments neither"   

source:    "//this is not a comment, it's in the string \\", but //those are comments since the string is terminated   
target:    "//this is not a comment, it's in the string \\", but 

source:    '//this is not a comment, it's in the string, // but, only for the first part and those are comments since the string is terminated   
target:    '//this is not a comment, it's in the string, 

source:    "//this is not a comment, it's in the string ", and '//those are not comments as well' since in the //single qoutes."   
target:    "//this is not a comment, it's in the string ", and '//those are not comments as well' since in the 

source:    /*this is a comment*/ my actrual data   
target:     my actrual data   

source:    /**/ my actrual data /**/  
target:     my actrual data   

source:    /**/ my actr/**/ual data /**/  
target:     my actrual data   

source:    /**/ my actr/****////**/**///**/**/ual data /**/  
target:     my actr

source:    '//this is not a comment, it's i/**/n the st/**/ring, // but, on/**/ly for the first part and those are comments since the string is terminated   
target:    '//this is not a comment, it's in the string, 

source:    "//this is not a comment, it's /*in the string*/ ", and '//those are not comments as well' since in the single qoutes./* and in the comments */
target:    "//this is not a comment, it's /*in the string*/ ", and '//those are not comments as well' since in the single qoutes.

```

# License:
PureSource is using MIT

