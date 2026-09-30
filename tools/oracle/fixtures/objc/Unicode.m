#import <Foundation/Foundation.h>

@interface Grüße : NSObject
@property (nonatomic, copy) NSString *décor;
- (NSString *)grüße:(NSString *)名前;
@end

@implementation Grüße
- (NSString *)grüße:(NSString *)名前 {
    return [NSString stringWithFormat:@"héllo wörld %@ %@", self.décor, 名前];
}
@end
