#include <vector>

int main() {
    std::vector<char> v;
    // Allocate memory until MLE
    while (true) {
        v.resize(v.size() + 1024 * 1024); // 1MB at a time
    }
    return 0;
}
