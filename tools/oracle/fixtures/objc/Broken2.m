#import <Foundation/Foundation.h>

@interface ValidThing : NSObject
- (int)validMethod:(int)x;
@end

@implementation ValidThing
- (int)validMethod:(int)x {
    return x * 2;
}
@end

@implementation BrokenThing
- (void)unfinishedMethod {
    if (self) {
        [self doSomething
@end
