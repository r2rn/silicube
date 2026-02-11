#include <iostream>
#include <string>

int main() {
    int score = 0;

    std::cout << "Welcome to the quiz!" << std::endl;
    std::cout << "You will answer 3 questions." << std::endl;
    std::cout << "" << std::endl;

    std::cout << "Q1: What is 2+2?" << std::endl;
    int ans1;
    std::cin >> ans1;
    if (ans1 == 4) {
        std::cout << "Correct!" << std::endl;
        score++;
    } else {
        std::cout << "Wrong! The answer is 4." << std::endl;
    }

    std::cout << "" << std::endl;
    std::cout << "Q2: What is 3*5?" << std::endl;
    int ans2;
    std::cin >> ans2;
    if (ans2 == 15) {
        std::cout << "Correct!" << std::endl;
        score++;
    } else {
        std::cout << "Wrong! The answer is 15." << std::endl;
    }

    std::cout << "" << std::endl;
    std::cout << "Q3: What is 10-7?" << std::endl;
    int ans3;
    std::cin >> ans3;
    if (ans3 == 3) {
        std::cout << "Correct!" << std::endl;
        score++;
    } else {
        std::cout << "Wrong! The answer is 3." << std::endl;
    }

    std::cout << "" << std::endl;
    std::cout << "Final score: " << score << "/3" << std::endl;

    return 0;
}
