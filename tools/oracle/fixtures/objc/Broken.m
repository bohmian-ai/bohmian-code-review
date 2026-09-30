#import <Foundation/Foundation.h>

@interface BrokenClass : NSObject
- (void)unterminated;
@end

@implementation BrokenClass
- (void)unterminated {
    if (self) {
        [self doSomething
    }
@end
