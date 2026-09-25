#import <Foundation/Foundation.h>
#import "LocalHelper.h"

@interface Greeter : NSObject <GreetingProtocol, NSSecureCoding>
@property (nonatomic, strong) NSString *prefix;
- (NSString *)greet:(NSString *)name withSuffix:(NSString *)suffix;
+ (instancetype)sharedGreeter;
- (void)reset;
@end
typedef enum {
    GreetingKindHello,
    GreetingKindGoodbye,
} GreetingKind;

typedef struct {
    int x;
    int y;
} Point;

@implementation Greeter
+ (instancetype)sharedGreeter {
    static Greeter *shared = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
        shared = [[Greeter alloc] init];
    });
    return shared;
}

- (NSString *)greet:(NSString *)name withSuffix:(NSString *)suffix {
    return [NSString stringWithFormat:@"%@, %@%@", self.prefix, name, suffix];
}
- (void)reset {
    self.prefix = nil;
}
@end

int global_counter = 0;

static void helper_function(int value) {
    global_counter += value;
}
