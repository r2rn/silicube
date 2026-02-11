#include <iostream>
#include <string>

int main() {
    std::string name;
    std::cout << "What is your name?" << std::endl;
    std::getline(std::cin, name);
    std::cout << "Hello, " << name << "!" << std::endl;

    int num;
    std::cout << "Enter a number:" << std::endl;
    std::cin >> num;
    std::cout << "Double: " << (num * 2) << std::endl;
    std::cout << "Triple: " << (num * 3) << std::endl;

    std::string word;
    std::cin.ignore();
    std::cout << "Enter a word:" << std::endl;
    std::getline(std::cin, word);
    std::cout << "You said: " << word << std::endl;
    std::cout << "Done!" << std::endl;

    return 0;
}
